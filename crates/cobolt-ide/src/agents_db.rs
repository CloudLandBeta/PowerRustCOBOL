// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Project agent database (spec 028).
//!
//! A project can define any number of AI agents under `agentic_ai/<name>/`,
//! each with its own model, prompt, capabilities, and knowledge, following
//! the operator's structure:
//!
//! ```text
//! agentic_ai/<agent_name>/
//! ├── <agent_name>_prompt.md   ← Core Instructions · agent prompt
//! ├── steering/                ← Core Instructions · steering files
//! ├── policies.md              ← Core Instructions · policies & constraints
//! ├── skills/                  ← Capabilities · skills
//! ├── mcp.json                 ← Capabilities · tools / MCP server definitions
//! ├── knowledge/               ← Knowledge · references, examples, domain docs
//! └── agent.json               ← Identity + Runtime config — NEVER the API key
//! ```
//!
//! Rules (all operator-decided): agent names are unique in the project and
//! IMMUTABLE (they name the folder and prompt file); API keys are asked per
//! model and stored machine-global in [`crate::llm::LlmConfig::api_keys`],
//! never in the project; a primary agent may name one pedantic companion,
//! and that pair must use different models (unrelated agents may share
//! models freely).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What an agent IS in the mesh (spec 029 R3). Stored — roles no longer
/// derive from companion links alone.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    /// Grace — the single coordination authority. Exactly one per project.
    Orchestrator,
    /// A worker agent owning a technical responsibility.
    #[default]
    Specialist,
    /// A reviewer companion; the only valid kind for `companion` links.
    Pedantic,
}

/// The orchestrator's one and only name (spec 029 R1).
pub const GRACE: &str = "Grace";

/// Canonical name of the COBOL event-handler specialist. Fixed because Grace
/// and the pedantic prompts delegate to it by this exact name.
pub const EVENT_HANDLER: &str = "COBOL Event Handler Script Agent";

/// Canonical name of the Git/version-control specialist for the project repo.
pub const VERSION_CONTROL: &str = "Version Control Agent";

/// Canonical name of the form-design specialist. Fixed because Grace delegates
/// form work to it by this exact name (spec 030 applies its approved output).
pub const FORM_DESIGNER: &str = "Form Designer Agent";

/// One agent's identity + runtime configuration (`agent.json`). The prompt
/// text deliberately lives outside this struct, in `<name>_prompt.md`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDef {
    /// UUID v4, generated at creation. The stable internal key.
    pub id: String,
    /// Unique, immutable; names the folder and prompt file.
    pub name: String,
    /// Orchestrator / specialist / pedantic (spec 029). Old manifests
    /// default to specialist.
    #[serde(default)]
    pub kind: AgentKind,
    /// Capability tag Grace selects by (e.g. "form-design", "cobol-events",
    /// "cobol-dev", "security", "documentation"). Free-form.
    #[serde(default)]
    pub specialization: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    // ── Runtime configuration ────────────────────────────────────────────
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub endpoint: String,
    /// Official model id passed in each request.
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    /// Reference to a global [`ModelProfile`](crate::llm::ModelProfile) id
    /// (spec 031). When set, the agent's runtime config resolves from that
    /// profile; the embedded `provider`/`endpoint`/`model`/params above are then
    /// dormant (kept only for migration + rollback). `None` on old manifests.
    #[serde(default)]
    pub model_profile: Option<String>,
    /// Free-text routing description (who calls it / whom it calls).
    #[serde(default)]
    pub routing: String,
    /// Agent id of the pedantic companion gating this agent's responses.
    #[serde(default)]
    pub companion: Option<String>,
    // ── Core instructions (file lists relative to the agent folder) ─────
    #[serde(default)]
    pub steering: Vec<String>,
    #[serde(default)]
    pub policies: Vec<String>,
    // ── Capabilities ─────────────────────────────────────────────────────
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    // ── Knowledge ────────────────────────────────────────────────────────
    #[serde(default)]
    pub knowledge: Vec<String>,
}

fn default_true() -> bool {
    true
}
fn default_temperature() -> f32 {
    0.4
}
fn default_max_tokens() -> u32 {
    8192
}
fn default_timeout() -> u32 {
    120
}

/// UUID v4 from the existing `rand` dependency (no extra crate).
pub fn new_uuid() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13],
        b[14], b[15]
    )
}

/// A valid agent name: non-empty, no path separators, no leading dot — it
/// becomes a directory name verbatim.
pub fn valid_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty()
        && !n.starts_with('.')
        && !n.contains('/')
        && !n.contains('\\')
        && !n.contains(':')
}

/// The project's agent database: every `agentic_ai/*/agent.json`.
#[derive(Debug, Default, Clone)]
pub struct AgentsDb {
    pub agents: Vec<AgentDef>,
    root: PathBuf,
}

impl AgentsDb {
    /// Load every agent under `<project>/agentic_ai/*/agent.json`. Agents
    /// are sorted primaries-first, each followed by its companion (the
    /// order the manager's rail shows).
    pub fn load(project_dir: &Path) -> Self {
        let root = crate::agent::project_agentic_root(project_dir);
        let mut agents = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten() {
                let manifest = e.path().join("agent.json");
                if let Ok(text) = std::fs::read_to_string(&manifest) {
                    if let Ok(a) = serde_json::from_str::<AgentDef>(&text) {
                        agents.push(a);
                    }
                }
            }
        }
        let mut db = Self { agents, root };
        db.sort_rail();
        db
    }

    /// Rail order: primaries alphabetically, each companion directly after
    /// its primary; orphan companions trail at the end.
    pub fn sort_rail(&mut self) {
        let companions: Vec<String> = self
            .agents
            .iter()
            .filter_map(|a| a.companion.clone())
            .collect();
        let mut primaries: Vec<AgentDef> = self
            .agents
            .iter()
            .filter(|a| !companions.contains(&a.id))
            .cloned()
            .collect();
        primaries.sort_by(|a, b| {
            // Grace (the orchestrator) is always pinned first.
            let oa = a.kind != AgentKind::Orchestrator;
            let ob = b.kind != AgentKind::Orchestrator;
            oa.cmp(&ob)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        let mut out = Vec::new();
        for p in primaries {
            let comp = p
                .companion
                .as_ref()
                .and_then(|cid| self.agents.iter().find(|x| &x.id == cid).cloned());
            out.push(p);
            if let Some(c) = comp {
                out.push(c);
            }
        }
        for a in &self.agents {
            if !out.iter().any(|x| x.id == a.id) {
                out.push(a.clone());
            }
        }
        self.agents = out;
    }

    /// `true` when `id` is some other agent's companion.
    pub fn is_companion(&self, id: &str) -> bool {
        self.agents
            .iter()
            .any(|a| a.companion.as_deref() == Some(id))
    }

    pub fn by_id(&self, id: &str) -> Option<&AgentDef> {
        self.agents.iter().find(|a| a.id == id)
    }

    pub fn by_name(&self, name: &str) -> Option<&AgentDef> {
        self.agents
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name.trim()))
    }

    fn dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn prompt_path(&self, name: &str) -> PathBuf {
        self.dir(name).join(format!("{name}_prompt.md"))
    }

    pub fn load_prompt(&self, name: &str) -> String {
        std::fs::read_to_string(self.prompt_path(name)).unwrap_or_default()
    }

    pub fn save_prompt(&self, name: &str, text: &str) -> Result<(), String> {
        std::fs::create_dir_all(self.dir(name)).map_err(|e| e.to_string())?;
        std::fs::write(self.prompt_path(name), text).map_err(|e| e.to_string())
    }

    /// Create a new agent: unique valid name, folder scaffold per spec 028
    /// R2, manifest + prompt written. Returns the new agent's id.
    pub fn create(&mut self, name: &str, prompt: &str) -> Result<String, String> {
        self.create_kinded(name, prompt, AgentKind::Specialist, "")
    }

    pub fn create_kinded(
        &mut self,
        name: &str,
        prompt: &str,
        kind: AgentKind,
        specialization: &str,
    ) -> Result<String, String> {
        let name = name.trim();
        if !valid_name(name) {
            return Err("The agent needs a valid name (no / \\ : or leading dot) — it names the folder agentic_ai/<name>/.".into());
        }
        if self.by_name(name).is_some() {
            return Err(format!(
                "An agent named \u{201c}{name}\u{201d} already exists — agent names must be unique in the project."
            ));
        }
        if kind != AgentKind::Orchestrator && name.eq_ignore_ascii_case(GRACE) {
            return Err("The name Grace is reserved for the orchestrator.".into());
        }
        let def = AgentDef {
            id: new_uuid(),
            name: name.to_string(),
            kind,
            specialization: specialization.to_string(),
            purpose: String::new(),
            enabled: true,
            provider: String::new(),
            endpoint: String::new(),
            model: String::new(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            timeout_secs: default_timeout(),
            model_profile: None,
            routing: String::new(),
            companion: None,
            steering: Vec::new(),
            policies: Vec::new(),
            skills: Vec::new(),
            tools: Vec::new(),
            knowledge: Vec::new(),
        };
        let dir = self.dir(name);
        for sub in ["steering", "skills", "knowledge"] {
            std::fs::create_dir_all(dir.join(sub)).map_err(|e| e.to_string())?;
        }
        for (file, contents) in [("policies.md", "# Policies and constraints\n"), ("mcp.json", "{}\n")] {
            let p = dir.join(file);
            if !p.exists() {
                std::fs::write(&p, contents).map_err(|e| e.to_string())?;
            }
        }
        self.save_prompt(name, prompt)?;
        let id = def.id.clone();
        self.save_agent(&def)?;
        self.agents.push(def);
        self.sort_rail();
        Ok(id)
    }

    /// Write one agent's `agent.json` (identity + runtime — never a key).
    pub fn save_agent(&self, def: &AgentDef) -> Result<(), String> {
        let dir = self.dir(&def.name);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(def).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("agent.json"), json).map_err(|e| e.to_string())
    }

    /// Persist every agent manifest.
    pub fn save_all(&self) -> Result<(), String> {
        for a in &self.agents {
            self.save_agent(a)?;
        }
        Ok(())
    }

    /// Delete an agent: folder removed, links to it cleared.
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let Some(a) = self.by_id(id).cloned() else {
            return Ok(());
        };
        let _ = std::fs::remove_dir_all(self.dir(&a.name));
        self.agents.retain(|x| x.id != id);
        for x in &mut self.agents {
            if x.companion.as_deref() == Some(id) {
                x.companion = None;
            }
        }
        self.save_all()
    }

    /// Spec 028 R5: a primary and ITS pedantic companion must use different
    /// models. Returns the first violation as (primary, companion) names.
    pub fn pair_rule_violation(&self) -> Option<(String, String)> {
        for a in &self.agents {
            if let Some(c) = a.companion.as_ref().and_then(|cid| self.by_id(cid)) {
                if !a.model.trim().is_empty()
                    && a.provider.trim() == c.provider.trim()
                    && a.model.trim() == c.model.trim()
                {
                    return Some((a.name.clone(), c.name.clone()));
                }
            }
        }
        None
    }

    /// First enabled agent that needs a key but has none stored (for the
    /// settings summary row).
    pub fn missing_key(&self, llm: &crate::llm::LlmConfig) -> Option<String> {
        self.agents
            .iter()
            .find(|a| {
                a.enabled
                    && !a.model.trim().is_empty()
                    && !a.provider.trim().is_empty()
                    && !a.provider.to_lowercase().contains("ollama")
                    && !llm
                        .api_keys
                        .contains_key(&crate::llm::api_key_slot(&a.provider, &a.model))
            })
            .map(|a| a.name.clone())
    }

    /// Spec 029 R1: create (or repair) the Grace orchestrator singleton.
    /// Idempotent; returns true when Grace was created this call.
    pub fn ensure_grace(&mut self) -> bool {
        if self
            .agents
            .iter()
            .any(|a| a.kind == AgentKind::Orchestrator)
        {
            return false;
        }
        match self.create_kinded(
            GRACE,
            &crate::llm::default_grace_prompt(),
            AgentKind::Orchestrator,
            "orchestration",
        ) {
            Ok(id) => {
                if let Some(g) = self.agents.iter_mut().find(|a| a.id == id) {
                    g.purpose =
                        "Central coordination authority: plans, delegates, enforces reviews, integrates."
                            .to_string();
                    g.routing = "Receives: every multi-agent request · Delegates to: all specialists".to_string();
                    g.temperature = 0.2;
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }

    /// Spec 029: exactly one orchestrator, and it must be named Grace.
    pub fn orchestrator_violation(&self) -> Option<String> {
        let orchs: Vec<&AgentDef> = self
            .agents
            .iter()
            .filter(|a| a.kind == AgentKind::Orchestrator)
            .collect();
        match orchs.as_slice() {
            [] => Some("missing".into()),
            [one] if one.name == GRACE => None,
            [one] => Some(one.name.clone()),
            _ => Some("multiple".into()),
        }
    }

    /// Spec 028 R7: seed the database from the legacy fixed-pair settings
    /// the first time a project opens with no agents. Returns how many
    /// agents were created.
    pub fn seed_from_legacy(&mut self, llm: &crate::llm::LlmConfig) -> usize {
        if !self.agents.is_empty() || !llm.is_configured() {
            return 0;
        }
        let mut created = 0;
        if self.ensure_grace() {
            created += 1;
        }
        if let Ok(designer_id) = self.create_kinded(
            "Form Designer Agent",
            &llm.system_prompt,
            AgentKind::Specialist,
            "form-design",
        ) {
            created += 1;
            if let Some(d) = self.agents.iter_mut().find(|a| a.id == designer_id) {
                d.purpose =
                    "Designs and edits forms; delegates event handlers (Phase 2).".to_string();
                d.provider = llm.provider.clone();
                d.endpoint = llm.endpoint.clone();
                d.model = llm.model.clone();
                d.temperature = llm.temperature;
                d.max_tokens = llm.max_tokens;
                d.timeout_secs = llm.timeout_secs;
                d.routing = "Receives: user form requests".to_string();
                // Observe-only live-UI tools (spec 030 R4/R5). Design edits still
                // go through the reviewable change-set path, not the live UI.
                d.tools = vec!["egui.tree".to_string(), "egui.rects".to_string()];
            }
            if llm.reviewer_configured() {
                if let Ok(ped_id) = self.create_kinded(
                    "Pedantic UI Agent",
                    &llm.pedantic_ui_prompt,
                    AgentKind::Pedantic,
                    "ui-review",
                ) {
                    created += 1;
                    if let Some(p) = self.agents.iter_mut().find(|a| a.id == ped_id) {
                        p.purpose =
                            "Uncompromising reviewer of every Form Designer result.".to_string();
                        p.provider = llm.reviewer_provider.clone();
                        p.endpoint = llm.reviewer_endpoint.clone();
                        p.model = llm.reviewer_model.clone();
                        p.temperature = 0.0;
                        p.routing = "Reviews: Form Designer Agent".to_string();
                    }
                    if let Some(d) = self.agents.iter_mut().find(|a| a.id == designer_id) {
                        d.companion = Some(ped_id);
                    }
                }
            }
            // COBOL Event Handler Script Agent — a fixed specialist seeded with
            // its own prompt and the same connection as the designer (one legacy
            // connection; specialists may share a model). Gets its Pedantic COBOL
            // companion when a reviewer model is configured.
            if let Ok(ev_id) = self.create_kinded(
                EVENT_HANDLER,
                &crate::llm::default_event_handler_prompt(),
                AgentKind::Specialist,
                "cobol-events",
            ) {
                created += 1;
                if let Some(e) = self.agents.iter_mut().find(|a| a.id == ev_id) {
                    e.purpose =
                        "Implements delegated COBOL-85 / RustCOBOL event handlers.".to_string();
                    e.provider = llm.provider.clone();
                    e.endpoint = llm.endpoint.clone();
                    e.model = llm.model.clone();
                    e.temperature = llm.temperature;
                    e.max_tokens = llm.max_tokens;
                    e.timeout_secs = llm.timeout_secs;
                    e.routing = "Receives: delegations from Form Designer Agent".to_string();
                }
                if llm.reviewer_configured() {
                    if let Ok(pc_id) = self.create_kinded(
                        "Pedantic COBOL Companion",
                        &llm.pedantic_event_prompt,
                        AgentKind::Pedantic,
                        "cobol-review",
                    ) {
                        created += 1;
                        if let Some(p) = self.agents.iter_mut().find(|a| a.id == pc_id) {
                            p.purpose =
                                "Reviews every event-handler implementation before completion."
                                    .to_string();
                            p.provider = llm.reviewer_provider.clone();
                            p.endpoint = llm.reviewer_endpoint.clone();
                            p.model = llm.reviewer_model.clone();
                            p.temperature = 0.0;
                            p.routing = "Reviews: COBOL Event Handler Script Agent".to_string();
                        }
                        if let Some(e) = self.agents.iter_mut().find(|a| a.id == ev_id) {
                            e.companion = Some(pc_id);
                        }
                    }
                }
            }
            // Version Control Agent — a fixed Git specialist for the project
            // repo, seeded with its prompt and the same connection. No pedantic
            // companion by default (git ops are executed, not code-reviewed).
            if let Ok(vc_id) = self.create_kinded(
                VERSION_CONTROL,
                &crate::llm::default_version_control_prompt(),
                AgentKind::Specialist,
                "version-control",
            ) {
                created += 1;
                if let Some(v) = self.agents.iter_mut().find(|a| a.id == vc_id) {
                    v.purpose =
                        "Manages Git for the project: branches, commits, push, revert, rebase."
                            .to_string();
                    v.provider = llm.provider.clone();
                    v.endpoint = llm.endpoint.clone();
                    v.model = llm.model.clone();
                    v.temperature = llm.temperature;
                    v.max_tokens = llm.max_tokens;
                    v.timeout_secs = llm.timeout_secs;
                    v.tools = vec!["git (project repository)".to_string(), "git.run".to_string()];
                    v.routing = "Delegated by Grace · version-control for the project repo".to_string();
                }
            }
            let _ = self.save_all();
            self.sort_rail();
        }
        created
    }

    /// Ensure the Version Control Agent exists (repairs databases seeded before
    /// it was added). Templates the connection off the Form Designer Agent.
    pub fn ensure_version_control(&mut self, llm: &crate::llm::LlmConfig) -> bool {
        if self.by_name(VERSION_CONTROL).is_some() {
            return false;
        }
        let tmpl = self.by_name("Form Designer Agent").cloned();
        let (provider, endpoint, model, temperature, max_tokens, timeout_secs) = match tmpl {
            Some(d) => (
                d.provider,
                d.endpoint,
                d.model,
                d.temperature,
                d.max_tokens,
                d.timeout_secs,
            ),
            None => (
                llm.provider.clone(),
                llm.endpoint.clone(),
                llm.model.clone(),
                llm.temperature,
                llm.max_tokens,
                llm.timeout_secs,
            ),
        };
        match self.create_kinded(
            VERSION_CONTROL,
            &crate::llm::default_version_control_prompt(),
            AgentKind::Specialist,
            "version-control",
        ) {
            Ok(id) => {
                if let Some(v) = self.agents.iter_mut().find(|a| a.id == id) {
                    v.purpose =
                        "Manages Git for the project: branches, commits, push, revert, rebase."
                            .to_string();
                    v.provider = provider;
                    v.endpoint = endpoint;
                    v.model = model;
                    v.temperature = temperature;
                    v.max_tokens = max_tokens;
                    v.timeout_secs = timeout_secs;
                    v.tools = vec!["git (project repository)".to_string(), "git.run".to_string()];
                    v.routing = "Delegated by Grace · version-control for the project repo".to_string();
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }

    /// Ensure the COBOL Event Handler Script Agent exists (repairs databases
    /// seeded before it was added). Uses the Form Designer Agent's connection
    /// as a template, else the legacy config. Returns true when created.
    pub fn ensure_event_handler(&mut self, llm: &crate::llm::LlmConfig) -> bool {
        if self.by_name(EVENT_HANDLER).is_some() {
            return false;
        }
        let tmpl = self.by_name("Form Designer Agent").cloned();
        let (provider, endpoint, model, temperature, max_tokens, timeout_secs) = match tmpl {
            Some(d) => (
                d.provider,
                d.endpoint,
                d.model,
                d.temperature,
                d.max_tokens,
                d.timeout_secs,
            ),
            None => (
                llm.provider.clone(),
                llm.endpoint.clone(),
                llm.model.clone(),
                llm.temperature,
                llm.max_tokens,
                llm.timeout_secs,
            ),
        };
        match self.create_kinded(
            EVENT_HANDLER,
            &crate::llm::default_event_handler_prompt(),
            AgentKind::Specialist,
            "cobol-events",
        ) {
            Ok(id) => {
                if let Some(e) = self.agents.iter_mut().find(|a| a.id == id) {
                    e.purpose =
                        "Implements delegated COBOL-85 / RustCOBOL event handlers.".to_string();
                    e.provider = provider;
                    e.endpoint = endpoint;
                    e.model = model;
                    e.temperature = temperature;
                    e.max_tokens = max_tokens;
                    e.timeout_secs = timeout_secs;
                    e.routing = "Receives: delegations from Form Designer Agent".to_string();
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }
}

/// Spec 028 R8 (Phase 1): resolve the designer agent's effective connection.
/// When a "Form Designer Agent" DB entry exists, is enabled, and names a
/// model, it overrides the legacy config (key restored from the machine-
/// global per-model store); otherwise the legacy config passes through.
pub fn designer_agent_config(db: &AgentsDb, llm: &crate::llm::LlmConfig) -> crate::llm::LlmConfig {
    agent_effective_config(db, llm, "Form Designer Agent").unwrap_or_else(|| llm.clone())
}

/// Resolve any enabled, model-configured agent by name into an `LlmConfig`:
/// its provider/endpoint/model/key/sampling, its prompt as the system prompt,
/// and its pedantic companion mapped into the reviewer fields (so a tandem
/// run — e.g. the COBOL proficiency check — reviews through it). Returns
/// `None` when the agent is absent, disabled, or has no model.
pub fn agent_effective_config(
    db: &AgentsDb,
    llm: &crate::llm::LlmConfig,
    agent_name: &str,
) -> Option<crate::llm::LlmConfig> {
    let a = db.by_name(agent_name).filter(|a| a.enabled)?;
    let mut cfg = resolve_agent_connection(a, llm)?;
    let prompt = db.load_prompt(&a.name);
    if !prompt.trim().is_empty() {
        cfg.system_prompt = prompt;
    }
    // The agent's pedantic companion (when set) IS the reviewer: the COBOL
    // proficiency check and every tandem loop resolve it from here. The
    // companion's connection resolves from ITS OWN profile (spec 031 R12).
    let companion = a
        .companion
        .as_ref()
        .and_then(|cid| db.by_id(cid))
        .filter(|c| c.enabled)
        .and_then(|c| resolve_agent_connection(c, llm).map(|cc| (c, cc)));
    match companion {
        Some((c, cc)) => {
            cfg.reviewer_provider = cc.provider.clone();
            cfg.reviewer_endpoint = cc.endpoint.clone();
            cfg.reviewer_model = cc.model.clone();
            let ped_prompt = db.load_prompt(&c.name);
            if !ped_prompt.trim().is_empty() {
                cfg.pedantic_prompt = ped_prompt;
            }
        }
        None => {
            // No usable companion: the run is explicitly unreviewed (the
            // caller warns the user — unreviewed responses can be useless).
            cfg.reviewer_provider.clear();
            cfg.reviewer_endpoint.clear();
            cfg.reviewer_model.clear();
        }
    }
    Some(cfg)
}

/// Resolve an agent's connection (provider/endpoint/model/params + key) into an
/// [`LlmConfig`] view (spec 031). Prefers the referenced **model profile**;
/// falls back to the agent's dormant embedded fields for agents not yet
/// migrated, so resolution never hard-breaks on upgrade. `None` when neither is
/// configured (a clear "no model" state — spec 031 R8).
pub fn resolve_agent_connection(
    a: &AgentDef,
    llm: &crate::llm::LlmConfig,
) -> Option<crate::llm::LlmConfig> {
    // A set profile reference is authoritative: if it dangles (the profile was
    // deleted), the agent is "no model configured" (R8) — we do NOT silently
    // fall back to stale embedded config in that case.
    if let Some(id) = a.model_profile.as_ref() {
        return llm.profile(id).map(|p| p.resolve(llm));
    }
    // No reference at all ⇒ un-migrated agent: fall back to its dormant embedded
    // fields so resolution never hard-breaks on upgrade.
    if !a.provider.trim().is_empty() && !a.model.trim().is_empty() {
        let mut cfg = llm.clone();
        cfg.provider = a.provider.clone();
        cfg.endpoint = a.endpoint.clone();
        cfg.model = a.model.clone();
        cfg.temperature = a.temperature;
        cfg.max_tokens = a.max_tokens;
        cfg.timeout_secs = a.timeout_secs;
        cfg.api_key = llm
            .api_keys
            .get(&crate::llm::api_key_slot(&a.provider, &a.model))
            .cloned()
            .unwrap_or_default();
        return Some(cfg);
    }
    None
}

/// Whether an agent has a usable model configured (a profile reference or
/// dormant embedded fields) — used for the "unreviewed" heuristic (spec 031).
fn agent_has_model(a: &AgentDef) -> bool {
    a.model_profile.is_some() || !a.model.trim().is_empty()
}

/// Migrate agents with embedded model config but no profile reference onto
/// synthesised global [`ModelProfile`](crate::llm::ModelProfile)s (spec 031 R6):
/// each distinct embedded connection becomes one profile (identical configs
/// collapse), and the agent is pointed at it. Idempotent — agents that already
/// reference a profile are left alone. Returns the number of agents migrated.
/// The caller persists `llm` and the agents afterwards.
pub fn migrate_to_profiles(db: &mut AgentsDb, llm: &mut crate::llm::LlmConfig) -> usize {
    let mut migrated = 0;
    for a in &mut db.agents {
        if a.model_profile.is_some() {
            continue;
        }
        if a.provider.trim().is_empty() || a.model.trim().is_empty() {
            continue; // nothing to synthesise from
        }
        let id = llm.find_or_create_profile(
            &a.provider,
            &a.endpoint,
            &a.model,
            a.temperature,
            a.max_tokens,
            a.timeout_secs,
        );
        a.model_profile = Some(id);
        migrated += 1;
    }
    migrated
}

/// Enabled primary agents (not companions of anyone) that have NO pedantic
/// companion — their responses ship unreviewed.
pub fn unreviewed_primaries(db: &AgentsDb) -> Vec<String> {
    db.agents
        .iter()
        .filter(|a| {
            a.kind == AgentKind::Specialist
                && a.enabled
                && !db.is_companion(&a.id)
                && a.companion
                    .as_ref()
                    .and_then(|cid| db.by_id(cid))
                    .map(|c| !c.enabled || !agent_has_model(c))
                    .unwrap_or(true)
        })
        .map(|a| a.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_project() -> PathBuf {
        let d = std::env::temp_dir().join(format!("prc_agents_{}", new_uuid()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn set_embedded(db: &mut AgentsDb, id: &str, provider: &str, endpoint: &str, model: &str) {
        let a = db.agents.iter_mut().find(|x| x.id == id).unwrap();
        a.provider = provider.into();
        a.endpoint = endpoint.into();
        a.model = model.into();
    }

    #[test]
    fn migration_synthesises_minimal_profiles_and_preserves_config() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        // A and B share an identical embedded connection; C differs.
        let a = db.create("A", "p").unwrap();
        let b = db.create("B", "p").unwrap();
        let c = db.create("C", "p").unwrap();
        set_embedded(&mut db, &a, "prov", "https://e", "m1");
        set_embedded(&mut db, &b, "prov", "https://e", "m1");
        set_embedded(&mut db, &c, "prov", "https://e", "m2");
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();

        // Effective model BEFORE migration (via embedded fallback).
        let before: Vec<String> = ["A", "B", "C"]
            .iter()
            .map(|n| agent_effective_config(&db, &llm, n).unwrap().model)
            .collect();

        let migrated = migrate_to_profiles(&mut db, &mut llm);
        assert_eq!(migrated, 3, "all three agents migrated");
        assert_eq!(llm.model_profiles.len(), 2, "identical configs collapse to 2 profiles");
        for n in ["A", "B", "C"] {
            assert!(db.by_name(n).unwrap().model_profile.is_some(), "{n} references a profile");
        }
        // Effective config is unchanged post-migration (AC5 invariant).
        let after: Vec<String> = ["A", "B", "C"]
            .iter()
            .map(|n| agent_effective_config(&db, &llm, n).unwrap().model)
            .collect();
        assert_eq!(before, after);
        // Idempotent.
        assert_eq!(migrate_to_profiles(&mut db, &mut llm), 0);
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn seeded_agents_migrate_onto_profiles_without_config_change() {
        let proj = tmp_project();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.provider = "anthropic".into();
        llm.model = "claude-sonnet-5".into();
        llm.endpoint = "https://api.anthropic.com/v1/messages".into();
        llm.reviewer_provider = "anthropic".into();
        llm.reviewer_model = "claude-opus-4-8".into();
        llm.reviewer_endpoint = llm.endpoint.clone();
        let mut db = AgentsDb::load(&proj);
        db.seed_from_legacy(&llm);

        let before = designer_agent_config(&db, &llm).model;
        let n = migrate_to_profiles(&mut db, &mut llm);
        assert!(n >= 4, "seeded specialists + companions migrated onto profiles");
        let d = db.by_name("Form Designer Agent").unwrap();
        assert!(d.model_profile.is_some(), "seeded Form Designer references a profile (AC7)");
        // Resolved config is unchanged, now via the profile.
        assert_eq!(designer_agent_config(&db, &llm).model, before);
        // No API key is ever stored in agent.json (AC4).
        assert!(!serde_json::to_string(d).unwrap().contains("api_key"));
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn deleted_profile_leaves_agent_without_model() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        let a = db.create("A", "p").unwrap();
        set_embedded(&mut db, &a, "prov", "https://e", "m1");
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        migrate_to_profiles(&mut db, &mut llm);
        assert!(agent_effective_config(&db, &llm, "A").is_some(), "resolves via its profile");
        // Delete the referenced profile → clear "no model" state, not a crash (R8).
        llm.model_profiles.clear();
        assert!(agent_effective_config(&db, &llm, "A").is_none());
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn create_scaffolds_the_operator_structure_and_round_trips() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        let id = db.create("Form Designer Agent", "line one\nline two\n").unwrap();
        let dir = proj.join("agentic_ai/Form Designer Agent");
        for p in [
            "agent.json",
            "Form Designer Agent_prompt.md",
            "policies.md",
            "mcp.json",
            "steering",
            "skills",
            "knowledge",
        ] {
            assert!(dir.join(p).exists(), "missing {p}");
        }
        // multi-line prompt survives verbatim
        assert_eq!(db.load_prompt("Form Designer Agent"), "line one\nline two\n");
        // manifest never contains a key field
        let json = std::fs::read_to_string(dir.join("agent.json")).unwrap();
        assert!(!json.to_lowercase().contains("api_key"), "key leaked: {json}");
        // reload sees the same agent
        let db2 = AgentsDb::load(&proj);
        assert_eq!(db2.agents.len(), 1);
        assert_eq!(db2.agents[0].id, id);
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn names_are_unique_and_validated() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        db.create("Agent A", "p").unwrap();
        assert!(db.create("agent a", "p").is_err(), "case-insensitive dup");
        assert!(db.create("", "p").is_err());
        assert!(db.create("a/b", "p").is_err());
        assert!(db.create(".hidden", "p").is_err());
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn pair_rule_only_binds_a_primary_to_its_own_companion() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        let a = db.create("Primary", "p").unwrap();
        let b = db.create("Companion", "p").unwrap();
        let c = db.create("Unrelated", "p").unwrap();
        for (id, model) in [(&a, "m1"), (&b, "m1"), (&c, "m1")] {
            let ag = db.agents.iter_mut().find(|x| &x.id == id).unwrap();
            ag.provider = "prov".into();
            ag.model = model.to_string();
        }
        // Same model everywhere but NO companion link: no violation.
        assert!(db.pair_rule_violation().is_none());
        // Link primary->companion with the same model: violation.
        db.agents.iter_mut().find(|x| x.id == a).unwrap().companion = Some(b.clone());
        assert!(db.pair_rule_violation().is_some());
        // Different model on the companion: fine again (Unrelated still shares m1).
        db.agents.iter_mut().find(|x| x.id == b).unwrap().model = "m2".into();
        assert!(db.pair_rule_violation().is_none());
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn seeding_migrates_the_legacy_pair() {
        let proj = tmp_project();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.provider = "anthropic".into();
        llm.model = "claude-sonnet-5".into();
        llm.endpoint = "https://api.anthropic.com/v1/messages".into();
        llm.reviewer_provider = "anthropic".into();
        llm.reviewer_model = "claude-opus-4-8".into();
        llm.reviewer_endpoint = llm.endpoint.clone();
        let mut db = AgentsDb::load(&proj);
        // Grace + designer + pedantic-ui + event-handler + pedantic-cobol +
        // version-control
        assert_eq!(db.seed_from_legacy(&llm), 6);
        let grace = db.by_name(GRACE).unwrap();
        assert_eq!(grace.kind, AgentKind::Orchestrator);
        assert!(db.orchestrator_violation().is_none());
        assert!(!db.load_prompt(GRACE).is_empty(), "Grace prompt seeded");
        // The COBOL Event Handler is a fixed specialist with its prompt loaded.
        let ev = db.by_name(EVENT_HANDLER).unwrap().clone();
        assert_eq!(ev.kind, AgentKind::Specialist);
        assert_eq!(ev.specialization, "cobol-events");
        assert!(!ev.model.trim().is_empty(), "event handler has a model");
        assert!(
            db.load_prompt(EVENT_HANDLER).contains("Event Handler"),
            "event-handler prompt loaded"
        );
        // Its pedantic companion was created and linked (reviewer configured).
        let pc = db.by_name("Pedantic COBOL Companion").unwrap();
        assert_eq!(pc.kind, AgentKind::Pedantic);
        assert_eq!(ev.companion.as_deref(), Some(pc.id.as_str()));
        assert!(db.pair_rule_violation().is_none());
        assert!(db.ensure_event_handler(&llm) == false, "idempotent");
        // The Version Control (Git) specialist is seeded with its prompt.
        let vc = db.by_name(VERSION_CONTROL).unwrap();
        assert_eq!(vc.kind, AgentKind::Specialist);
        assert_eq!(vc.specialization, "version-control");
        assert!(vc.companion.is_none(), "git agent has no auto companion");
        assert!(
            db.load_prompt(VERSION_CONTROL).contains("Version Control"),
            "version-control prompt loaded"
        );
        assert!(db.ensure_version_control(&llm) == false, "idempotent");
        let designer = db.by_name("Form Designer Agent").unwrap().clone();
        let ped = db.by_name("Pedantic UI Agent").unwrap();
        assert_eq!(designer.model, "claude-sonnet-5");
        assert_eq!(designer.companion.as_deref(), Some(ped.id.as_str()));
        assert_eq!(ped.model, "claude-opus-4-8");
        assert!(db.pair_rule_violation().is_none());
        // spec 030 R2: seeded agents declare the concrete tool names governance
        // recognises — VC gets git.run, the Form Designer gets the egui observers.
        assert!(
            db.by_name(VERSION_CONTROL).unwrap().tools.iter().any(|t| t == "git.run"),
            "VC declares git.run"
        );
        assert!(
            designer.tools.iter().any(|t| t == "egui.tree")
                && designer.tools.iter().any(|t| t == "egui.rects"),
            "Form Designer declares the egui observe tools"
        );
        // Second call is a no-op (agents exist).
        assert_eq!(db.seed_from_legacy(&llm), 0);
        // R8: the designer flow resolves to the DB entry, and the DB
        // companion becomes the reviewer (pedantic handles the check).
        let cfg = designer_agent_config(&db, &llm);
        assert_eq!(cfg.model, "claude-sonnet-5");
        assert_eq!(cfg.reviewer_model, "claude-opus-4-8");
        assert!(cfg.reviewer_configured());
        // Designer + event-handler are reviewed; only the companion-less Git
        // specialist is flagged unreviewed.
        assert_eq!(unreviewed_primaries(&db), vec![VERSION_CONTROL.to_string()]);
        // Remove the companion link: the primary is now unreviewed and the
        // resolved config carries NO reviewer (caller must warn the user).
        db.agents
            .iter_mut()
            .find(|a| a.name == "Form Designer Agent")
            .unwrap()
            .companion = None;
        let cfg = designer_agent_config(&db, &llm);
        assert!(!cfg.reviewer_configured());
        // Kinds are STORED (spec 029): the unlinked pedantic reviewer and
        // Grace are NOT unreviewed primaries — only companion-less specialists
        // are (now the designer AND the Git agent).
        let unreviewed = unreviewed_primaries(&db);
        assert!(unreviewed.contains(&"Form Designer Agent".to_string()));
        assert!(unreviewed.contains(&VERSION_CONTROL.to_string()));
        assert_eq!(unreviewed.len(), 2);
        // Grace is singleton + reserved.
        assert!(!db.ensure_grace(), "second ensure_grace is a no-op");
        assert!(db.create("grace", "p").is_err(), "name reserved");
        let _ = std::fs::remove_dir_all(proj);
    }
}
