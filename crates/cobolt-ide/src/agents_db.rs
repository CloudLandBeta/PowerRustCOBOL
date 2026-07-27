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

/// Canonical built-in reviewer companion for Grace's orchestration work.
pub const PEDANTIC_GRACE_REVIEWER: &str = "Grace Pedantic Reviewer";

/// Built-in tandem reviewer for the Form Designer specialist.
pub const PEDANTIC_FORM_DESIGNER_REVIEWER: &str = "Form Designer Agent Pedantic Reviewer";

/// Built-in tandem reviewer for the event-handler specialist.
pub const PEDANTIC_EVENT_HANDLER_REVIEWER: &str =
    "COBOL Event Handler Script Agent Pedantic Reviewer";

/// Built-in tandem reviewer for project documentation work.
pub const PEDANTIC_DOCUMENTATION_REVIEWER: &str = "Documentation Agent Pedantic Reviewer";

/// Built-in tandem reviewer for repository operations.
pub const PEDANTIC_VERSION_CONTROL_REVIEWER: &str = "Version Control Agent Pedantic Reviewer";
pub const DATA_INDEXED_FILE_AGENT: &str = "Data (Indexed File) Agent";
pub const PEDANTIC_DATA_INDEXED_FILE_REVIEWER: &str = "Data (Indexed File) Agent Pedantic Reviewer";

/// Canonical name of the COBOL event-handler specialist. Fixed because Grace
/// and the pedantic prompts delegate to it by this exact name.
pub const EVENT_HANDLER: &str = "COBOL Event Handler Script Agent";

/// Canonical name of the Git/version-control specialist for the project repo.
pub const VERSION_CONTROL: &str = "Version Control Agent";

/// Canonical name of the form-design specialist. Fixed because Grace delegates
/// form work to it by this exact name (spec 030 applies its approved output).
pub const FORM_DESIGNER: &str = "Form Designer Agent";

/// Fixed documentation specialist. Grace must delegate every project-document
/// creation or update to this exact agent.
pub const DOCUMENTATION_AGENT: &str = "Documentation Agent";

/// Whether an agent's approved submission is applied to the open form as a
/// change-set. Grace routes an event-only request straight to
/// [`EVENT_HANDLER`] (no form-design task is needed when every control already
/// exists), so that agent's output reaches the form by the same path the
/// designer's does. Membership here must stay in lockstep on both sides: an
/// agent in this set is told the change-set schema (`CHANGE_SET_CONTRACT`) and
/// is harvested by the apply path. Telling only one side is what silently
/// discards a reviewed, approved handler.
pub fn produces_form_change_set(agent: &str) -> bool {
    agent.eq_ignore_ascii_case(FORM_DESIGNER) || agent.eq_ignore_ascii_case(EVENT_HANDLER)
}

const LEGACY_PEDANTIC_GRACE_REVIEWER: &str = "Pedantic Grace Reviewer";
const LEGACY_ORCHESTRATOR_REVIEWER: &str = "Orchestrator Pedantic Reviewer Agent";
const LEGACY_GRACE_REVIEWER_AGENT: &str = "Grace Pedantic Reviewer Agent";
const LEGACY_FORM_DESIGNER_REVIEWER: &str = "Pedantic UI Agent";
const LEGACY_EVENT_HANDLER_REVIEWER: &str = "Pedantic COBOL Companion";
const LEGACY_DOCUMENTATION_AGENT: &str = "DocumentationAgent";

const FORM_DESIGNER_ROUTING: &str = "Preferred for RAD Form Designer requests; receives form design and change tasks from Grace; delegates event implementation to COBOL Event Handler Script Agent; supplies form facts to Documentation Agent; returns validated form change sets to Grace.";
const EVENT_HANDLER_ROUTING: &str = "Receives event implementation tasks from Grace or Form Designer Agent; returns complete COBOL-85/RustCOBOL handlers and validation evidence to Grace; performs no form design or documentation writes.";
const DOCUMENTATION_ROUTING: &str = "Receives Knowledge Base authoring tasks from Grace after authoritative domain-specialist handoff; writes only under Knowledge Base/ through documentation tools; retrieves project Knowledge Base evidence before model training, and for indexed-file work establishes file name and purpose, applies 1NF/2NF/3NF, requires the developer's UUID-or-PIC ID choice, and returns schema/helper-file requests to Grace without mutating .cidx files.";
const DATA_INDEXED_FILE_ROUTING: &str = "Receives indexed-file create/modify tasks only from Grace after an approved Documentation Agent schema handoff; uses indexed_file tools backed by the PowerRustCOBOL Indexed File UI model; returns validated .cidx and generated-artifact evidence to Grace; performs no documentation, form, event, or Git work.";
const GRACE_REVIEWER_ROUTING: &str = "Reviews only Grace's orchestration plans, delegation, evidence, integration, and final completion claims; returns an approval verdict or complete correction request to Grace.";
const VERSION_CONTROL_ROUTING: &str = "Receives project Git tasks from Grace or an explicit developer request; requires confirmation for destructive or history-changing operations; returns command evidence and repository state to Grace.";
const FORM_DESIGNER_REVIEWER_ROUTING: &str = "Reviews only Form Designer Agent output: controls, hierarchy, layout, styling, bindings, accessibility, event delegation, tool evidence, and regressions; returns an approval verdict or complete correction request to Grace.";
const EVENT_HANDLER_REVIEWER_ROUTING: &str = "Reviews only COBOL Event Handler Script Agent output for COBOL-85/RustCOBOL correctness, exact event integration, validation, state changes, error handling, and regression risk; returns an approval verdict or complete correction request to Grace.";
const DOCUMENTATION_REVIEWER_ROUTING: &str = "Reviews only Documentation Agent output for fidelity to approved specialist source material, required structure, allowed project path, technical accuracy, readability, indexability, and unsupported invention; returns an approval verdict or complete correction request to Grace.";
const VERSION_CONTROL_REVIEWER_ROUTING: &str = "Reviews only Version Control Agent output for requested scope, repository state, command safety, confirmation gates, evidence, reversibility, and unintended changes; returns an approval verdict or complete correction request to Grace.";
const DATA_INDEXED_FILE_REVIEWER_ROUTING: &str = "Reviews only Data (Indexed File) Agent output for approved schema handoff, explicit UUID-or-PIC choice, 1NF/2NF/3NF implementation, keys, PIC clauses, offsets, helper files, Indexed File UI tool evidence, generated artifacts, and regressions; returns an approval verdict or complete correction request to Grace.";

pub fn is_fixed_agent_name(name: &str) -> bool {
    name.eq_ignore_ascii_case(GRACE)
        || name.eq_ignore_ascii_case(PEDANTIC_GRACE_REVIEWER)
        || name.eq_ignore_ascii_case(PEDANTIC_FORM_DESIGNER_REVIEWER)
        || name.eq_ignore_ascii_case(PEDANTIC_EVENT_HANDLER_REVIEWER)
        || name.eq_ignore_ascii_case(PEDANTIC_DOCUMENTATION_REVIEWER)
        || name.eq_ignore_ascii_case(PEDANTIC_VERSION_CONTROL_REVIEWER)
        || name.eq_ignore_ascii_case(PEDANTIC_DATA_INDEXED_FILE_REVIEWER)
        || name.eq_ignore_ascii_case(FORM_DESIGNER)
        || name.eq_ignore_ascii_case(EVENT_HANDLER)
        || name.eq_ignore_ascii_case(VERSION_CONTROL)
        || name.eq_ignore_ascii_case(DOCUMENTATION_AGENT)
        || name.eq_ignore_ascii_case(DATA_INDEXED_FILE_AGENT)
        || name.eq_ignore_ascii_case(LEGACY_PEDANTIC_GRACE_REVIEWER)
        || name.eq_ignore_ascii_case(LEGACY_ORCHESTRATOR_REVIEWER)
        || name.eq_ignore_ascii_case(LEGACY_GRACE_REVIEWER_AGENT)
        || name.eq_ignore_ascii_case(LEGACY_FORM_DESIGNER_REVIEWER)
        || name.eq_ignore_ascii_case(LEGACY_EVENT_HANDLER_REVIEWER)
        || name.eq_ignore_ascii_case(LEGACY_DOCUMENTATION_AGENT)
}

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
    /// The developer explicitly chose **(no model)** for this agent. Without
    /// this marker an agent with no profile is indistinguishable from one that
    /// was never configured, and the built-in seeding
    /// ([`AgentsDb::ensure_fixed_agents`]) hands it a model back on the next
    /// project open — the choice would look like it never saved. Authoritative:
    /// it also suppresses the dormant embedded `provider`/`model` fallback.
    #[serde(default)]
    pub no_model: bool,
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

fn extend_unique(target: &mut Vec<String>, source: &[String]) {
    for value in source {
        if !target.contains(value) {
            target.push(value.clone());
        }
    }
}

/// Detect a stored built-in prompt still carrying the form-style guidance that
/// cannot work (spec 1.30.55). Agent prompts live as files inside each project,
/// so correcting the Rust defaults only helps NEW projects — an existing
/// project keeps teaching its agents to set `Theme` to an invented
/// "neumorphic-dark" id via `UPDATE_FORM_PROPERTY`, an operation the change-set
/// applier discards. Such a prompt is not a project customization worth
/// preserving; it is broken, so project-open repair replaces it wholesale.
/// True when `prompt` is a superseded shipped default, still unmodified.
///
/// `marker` is a string only the current default contains, so an already-upgraded
/// prompt is recognised without comparing it; `legacy` is the previous shipped
/// text, compared **exactly**. The exact comparison is the point: a developer who
/// edited their copy — even by one line — keeps it, and only an untouched old
/// default is replaced.
fn prompt_is_unmodified_legacy(prompt: &str, marker: &str, legacy: &str) -> bool {
    !prompt.contains(marker) && prompt.trim() == legacy.trim()
}

/// True when the Event Handler pair's prompts predate the RustCOBOL language
/// contract. The old prompts told the agent to "emit COBOL-85 conformant code"
/// and to "format COBOL strictly for the IDE parser" without ever saying which
/// verbs, intrinsics, levels or source format this toolchain accepts — so the
/// agent guessed, the reviewer had nothing concrete to check against, and
/// standard-looking COBOL that this parser rejects was the usual result.
fn prompt_predates_language_contract(name: &str, prompt: &str) -> bool {
    if name == EVENT_HANDLER {
        return prompt_is_unmodified_legacy(
            prompt,
            crate::llm::EVENT_HANDLER_LANGUAGE_CONTRACT_MARKER,
            crate::llm::LEGACY_EVENT_HANDLER_PROMPT_V1,
        );
    }
    if name == PEDANTIC_EVENT_HANDLER_REVIEWER {
        return prompt_is_unmodified_legacy(
            prompt,
            crate::llm::PEDANTIC_EVENT_LANGUAGE_CONTRACT_MARKER,
            crate::llm::LEGACY_PEDANTIC_EVENT_PROMPT_V1,
        );
    }
    false
}

fn prompt_carries_broken_form_style_guidance(prompt: &str) -> bool {
    // A corrected prompt always names the real property. Stale ones never do —
    // they only ever spoke of `Theme`. This keeps the corrected defaults from
    // matching their own warnings ("do NOT invent slugs such as …"), which they
    // must state to be useful.
    if prompt.contains("GlassStyle") {
        return false;
    }

    // Markers of superseded guidance: operation names the change-set applier
    // discards, invented theme slugs that never named a real style, and the
    // reviewer's demand for proof that a change was already applied — which
    // cannot be met, since change-sets are applied only after approval.
    const BROKEN_MARKERS: &[&str] = &[
        "UPDATE_FORM_PROPERTY",
        "UPDATE_CONTROL_PROPERTIES",
        "neumorphic-dark",
        "emerald-glass",
        "cobalt-steel",
        "The theme MUST be applied",
        "proving each change was applied",
    ];
    BROKEN_MARKERS.iter().any(|m| prompt.contains(m))
}

fn canonicalize_builtin_names(text: &str) -> String {
    text.replace(LEGACY_ORCHESTRATOR_REVIEWER, PEDANTIC_GRACE_REVIEWER)
        .replace(LEGACY_PEDANTIC_GRACE_REVIEWER, PEDANTIC_GRACE_REVIEWER)
        .replace(LEGACY_GRACE_REVIEWER_AGENT, PEDANTIC_GRACE_REVIEWER)
        .replace(
            LEGACY_FORM_DESIGNER_REVIEWER,
            PEDANTIC_FORM_DESIGNER_REVIEWER,
        )
        .replace(
            LEGACY_EVENT_HANDLER_REVIEWER,
            PEDANTIC_EVENT_HANDLER_REVIEWER,
        )
        .replace(LEGACY_DOCUMENTATION_AGENT, DOCUMENTATION_AGENT)
        .replace("/Documentation/", "/Knowledge Base/")
        .replace("/docs/", "/Knowledge Base/")
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
        db.normalize_companion_relationships();
        db.sort_rail();
        db
    }

    fn migrate_builtin_aliases(&mut self) -> usize {
        let mut changed = 0;
        for (legacy, canonical) in [
            (LEGACY_PEDANTIC_GRACE_REVIEWER, PEDANTIC_GRACE_REVIEWER),
            (LEGACY_ORCHESTRATOR_REVIEWER, PEDANTIC_GRACE_REVIEWER),
            (LEGACY_GRACE_REVIEWER_AGENT, PEDANTIC_GRACE_REVIEWER),
            (
                LEGACY_FORM_DESIGNER_REVIEWER,
                PEDANTIC_FORM_DESIGNER_REVIEWER,
            ),
            (
                LEGACY_EVENT_HANDLER_REVIEWER,
                PEDANTIC_EVENT_HANDLER_REVIEWER,
            ),
            (LEGACY_DOCUMENTATION_AGENT, DOCUMENTATION_AGENT),
        ] {
            if self.migrate_agent_alias(legacy, canonical) {
                changed += 1;
            }
        }
        changed
    }

    /// Rename a built-in agent without replacing its stable id or connection.
    /// When both names exist, merge useful configuration into the canonical
    /// record, relink owners, and remove the obsolete duplicate.
    fn migrate_agent_alias(&mut self, legacy: &str, canonical: &str) -> bool {
        let Some(source_index) = self
            .agents
            .iter()
            .position(|agent| agent.name.eq_ignore_ascii_case(legacy))
        else {
            return false;
        };
        let source = self.agents[source_index].clone();
        let source_prompt =
            canonicalize_builtin_names(&self.load_prompt(&source.name)).replace(legacy, canonical);

        if let Some(target_index) = self
            .agents
            .iter()
            .position(|agent| agent.name.eq_ignore_ascii_case(canonical))
        {
            let target_id = self.agents[target_index].id.clone();
            let target_prompt = canonicalize_builtin_names(&self.load_prompt(canonical));
            {
                let target = &mut self.agents[target_index];
                // An explicit "(no model)" is a choice, not an unconfigured
                // agent, and is never filled in from the legacy alias.
                let target_unconfigured = !target.no_model
                    && target.model_profile.is_none()
                    && target.provider.trim().is_empty()
                    && target.model.trim().is_empty();
                if target_unconfigured {
                    target.provider = source.provider.clone();
                    target.endpoint = source.endpoint.clone();
                    target.model = source.model.clone();
                    target.temperature = source.temperature;
                    target.max_tokens = source.max_tokens;
                    target.timeout_secs = source.timeout_secs;
                    target.model_profile = source.model_profile.clone();
                }
                if target.companion.is_none() {
                    target.companion = source.companion.clone();
                }
                if target.purpose.trim().is_empty() {
                    target.purpose = source.purpose.clone();
                }
                if target.routing.trim().is_empty() {
                    target.routing = source.routing.clone();
                }
                extend_unique(&mut target.steering, &source.steering);
                extend_unique(&mut target.policies, &source.policies);
                extend_unique(&mut target.skills, &source.skills);
                extend_unique(&mut target.tools, &source.tools);
                extend_unique(&mut target.knowledge, &source.knowledge);
            }
            for agent in &mut self.agents {
                if agent.companion.as_deref() == Some(source.id.as_str()) {
                    agent.companion = Some(target_id.clone());
                }
            }
            if target_prompt.trim().is_empty() && !source_prompt.trim().is_empty() {
                let _ = self.save_prompt(canonical, &source_prompt);
            } else if !target_prompt.trim().is_empty()
                && target_prompt != self.load_prompt(canonical)
            {
                let _ = self.save_prompt(canonical, &target_prompt);
            }
            self.agents.retain(|agent| agent.id != source.id);
            let _ = std::fs::remove_dir_all(self.dir(&source.name));
            let _ = self.save_all();
            return true;
        }

        let old_dir = self.dir(&source.name);
        let new_dir = self.dir(canonical);
        if new_dir.exists() || std::fs::rename(&old_dir, &new_dir).is_err() {
            return false;
        }
        self.agents[source_index].name = canonical.to_string();
        let moved_prompt = new_dir.join(format!("{}_prompt.md", source.name));
        let canonical_prompt = new_dir.join(format!("{canonical}_prompt.md"));
        if moved_prompt.exists() && !canonical_prompt.exists() {
            let _ = std::fs::rename(&moved_prompt, &canonical_prompt);
        }
        if !source_prompt.trim().is_empty() {
            let _ = self.save_prompt(canonical, &source_prompt);
        }
        let _ = self.save_agent(&self.agents[source_index]);
        true
    }

    /// Repair legacy many-to-one or invalid companion links in memory. The
    /// deterministic winner is the orchestrator first, then specialists by
    /// name. The next Agents Manager save persists the repaired manifests.
    fn normalize_companion_relationships(&mut self) -> bool {
        let pedantic_ids: Vec<String> = self
            .agents
            .iter()
            .filter(|agent| agent.kind == AgentKind::Pedantic)
            .map(|agent| agent.id.clone())
            .collect();
        let mut owner_indices: Vec<usize> = (0..self.agents.len()).collect();
        owner_indices.sort_by(|left, right| {
            let left_agent = &self.agents[*left];
            let right_agent = &self.agents[*right];
            let left_priority = left_agent.kind != AgentKind::Orchestrator;
            let right_priority = right_agent.kind != AgentKind::Orchestrator;
            left_priority
                .cmp(&right_priority)
                .then(
                    left_agent
                        .name
                        .to_lowercase()
                        .cmp(&right_agent.name.to_lowercase()),
                )
                .then(left_agent.id.cmp(&right_agent.id))
        });

        let mut changed = false;
        let mut claimed = Vec::<String>::new();
        for index in owner_indices {
            let companion = self.agents[index].companion.clone();
            let valid_owner = self.agents[index].kind != AgentKind::Pedantic;
            let valid_reviewer = companion
                .as_ref()
                .map(|id| pedantic_ids.contains(id))
                .unwrap_or(true);
            let duplicate = companion
                .as_ref()
                .map(|id| claimed.contains(id))
                .unwrap_or(false);
            if !valid_owner || !valid_reviewer || duplicate {
                if companion.is_some() {
                    self.agents[index].companion = None;
                    changed = true;
                }
            } else if let Some(id) = companion {
                claimed.push(id);
            }
        }
        changed
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

    /// The orchestrator or specialist currently reviewed by this Pedantic
    /// agent. The relationship is one-to-one.
    pub fn companion_owner(&self, reviewer_id: &str) -> Option<&AgentDef> {
        self.agents
            .iter()
            .find(|agent| agent.companion.as_deref() == Some(reviewer_id))
    }

    /// Set one orchestrator/specialist's Pedantic companion. A reviewer can
    /// belong to only one owner, so assigning it here detaches any prior owner.
    /// Persistence remains controlled by the caller's Apply/OK transaction.
    pub fn set_companion(
        &mut self,
        owner_id: &str,
        reviewer_id: Option<&str>,
    ) -> Result<bool, String> {
        let owner_index = self
            .agents
            .iter()
            .position(|agent| agent.id == owner_id)
            .ok_or_else(|| "The reviewed agent no longer exists.".to_string())?;
        if self.agents[owner_index].kind == AgentKind::Pedantic {
            return Err("A Pedantic agent cannot own a Pedantic companion.".into());
        }
        if let Some(reviewer_id) = reviewer_id {
            let reviewer = self
                .by_id(reviewer_id)
                .ok_or_else(|| "The Pedantic reviewer no longer exists.".to_string())?;
            if reviewer.kind != AgentKind::Pedantic {
                return Err(
                    "Only a Pedantic agent can be assigned as a companion reviewer.".into(),
                );
            }
        }

        let mut changed = false;
        if let Some(reviewer_id) = reviewer_id {
            for (index, agent) in self.agents.iter_mut().enumerate() {
                if index != owner_index && agent.companion.as_deref() == Some(reviewer_id) {
                    agent.companion = None;
                    changed = true;
                }
            }
        }
        if self.agents[owner_index].companion.as_deref() != reviewer_id {
            self.agents[owner_index].companion = reviewer_id.map(str::to_owned);
            changed = true;
        }
        Ok(changed)
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

    /// Load an agent's complete core instructions: prompt file + listed steering files + listed policy files.
    pub fn load_agent_core_instructions(&self, name: &str) -> String {
        let mut result = self.load_prompt(name);
        if let Some(agent) = self.by_name(name) {
            let dir = self.dir(&agent.name);
            for steering_file in &agent.steering {
                let p = dir.join(steering_file);
                let p_sub = dir.join("steering").join(steering_file);
                let content = std::fs::read_to_string(&p)
                    .or_else(|_| std::fs::read_to_string(&p_sub))
                    .unwrap_or_default();
                if !content.trim().is_empty() {
                    if !result.is_empty() {
                        result.push_str("\n\n");
                    }
                    result.push_str(&format!("--- STEERING ({}) ---\n{}", steering_file, content.trim()));
                }
            }
            for policy_file in &agent.policies {
                let p = dir.join(policy_file);
                let content = std::fs::read_to_string(&p).unwrap_or_default();
                if !content.trim().is_empty() {
                    if !result.is_empty() {
                        result.push_str("\n\n");
                    }
                    result.push_str(&format!("--- POLICIES ({}) ---\n{}", policy_file, content.trim()));
                }
            }
        }
        result
    }

    /// Load an agent's configured skills reference material.
    pub fn load_agent_capabilities(&self, name: &str) -> String {
        let mut parts = Vec::new();
        if let Some(agent) = self.by_name(name) {
            let dir = self.dir(&agent.name);
            for skill_file in &agent.skills {
                let p = dir.join(skill_file);
                let p_sub = dir.join("skills").join(skill_file);
                let content = std::fs::read_to_string(&p)
                    .or_else(|_| std::fs::read_to_string(&p_sub))
                    .unwrap_or_default();
                if !content.trim().is_empty() {
                    parts.push(format!("--- SKILL ({}) ---\n{}", skill_file, content.trim()));
                }
            }
        }
        parts.join("\n\n")
    }

    /// Load an agent's configured knowledge reference files.
    pub fn load_agent_knowledge(&self, name: &str) -> String {
        let mut parts = Vec::new();
        if let Some(agent) = self.by_name(name) {
            let dir = self.dir(&agent.name);
            for knowledge_file in &agent.knowledge {
                let p = dir.join(knowledge_file);
                let p_sub = dir.join("knowledge").join(knowledge_file);
                let content = std::fs::read_to_string(&p)
                    .or_else(|_| std::fs::read_to_string(&p_sub))
                    .unwrap_or_default();
                if !content.trim().is_empty() {
                    parts.push(format!("--- KNOWLEDGE ({}) ---\n{}", knowledge_file, content.trim()));
                }
            }
        }
        parts.join("\n\n")
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
            no_model: false,
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
        for (file, contents) in [
            ("policies.md", "# Policies and constraints\n"),
            ("mcp.json", "{}\n"),
        ] {
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
        if is_fixed_agent_name(&a.name) {
            return Err(format!(
                "{} is a fixed project agent and cannot be deleted.",
                a.name
            ));
        }
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
                if !a.enabled {
                    return false;
                }
                let Some(cfg) = resolve_agent_connection(a, llm) else {
                    return false;
                };
                !cfg.model.trim().is_empty()
                    && !cfg.provider.trim().is_empty()
                    && !cfg.provider.to_lowercase().contains("ollama")
                    && cfg.api_key.trim().is_empty()
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
                    g.routing =
                        "Receives: every multi-agent request · Delegates to: all specialists"
                            .to_string();
                    // temperature is always resolved from the model profile at call time;
                    // do not hardcode a default here.
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }

    /// Create, configure, and link one built-in tandem reviewer. Reviewer
    /// models are deliberately left unset for the developer to choose. Existing
    /// non-empty prompts, skills, tools, knowledge, and model choices survive
    /// repair. A previously linked reviewer is renamed to the tandem's canonical
    /// `<owner name> Pedantic Reviewer` name without rebuilding its configuration.
    fn ensure_tandem_reviewer(
        &mut self,
        owner_name: &str,
        reviewer_name: &str,
        specialization: &str,
        purpose: &str,
        routing: &str,
        default_prompt: &str,
    ) -> bool {
        let Some(mut owner) = self.by_name(owner_name).cloned() else {
            return false;
        };
        let mut changed = false;
        let noncanonical_companion = owner
            .companion
            .as_deref()
            .and_then(|id| self.by_id(id))
            .filter(|companion| {
                companion.kind == AgentKind::Pedantic
                    && !companion.name.eq_ignore_ascii_case(reviewer_name)
            })
            .map(|companion| companion.name.clone());
        if let Some(legacy_name) = noncanonical_companion {
            changed |= self.migrate_agent_alias(&legacy_name, reviewer_name);
            if let Some(refreshed) = self.by_name(owner_name).cloned() {
                owner = refreshed;
            }
        }

        let reviewer_id = if let Some(index) = self
            .agents
            .iter()
            .position(|agent| agent.name.eq_ignore_ascii_case(reviewer_name))
        {
            let id = self.agents[index].id.clone();
            let current_prompt = self.load_prompt(reviewer_name);
            if current_prompt.trim().is_empty()
                && self.save_prompt(reviewer_name, default_prompt).is_ok()
            {
                changed = true;
            }
            let reviewer = &mut self.agents[index];
            if reviewer.kind != AgentKind::Pedantic {
                reviewer.kind = AgentKind::Pedantic;
                changed = true;
            }
            if reviewer.specialization != specialization {
                reviewer.specialization = specialization.into();
                changed = true;
            }
            if reviewer.purpose.trim().is_empty() {
                reviewer.purpose = purpose.into();
                changed = true;
            }
            if reviewer.routing.trim().is_empty() {
                reviewer.routing = routing.into();
                changed = true;
            }
            id
        } else {
            let Ok(id) = self.create_kinded(
                reviewer_name,
                default_prompt,
                AgentKind::Pedantic,
                specialization,
            ) else {
                return false;
            };
            if let Some(reviewer) = self.agents.iter_mut().find(|agent| agent.id == id) {
                reviewer.purpose = purpose.into();
                reviewer.routing = routing.into();
                // temperature is always resolved from the model profile at call time;
                // we do not hardcode a default here.
            }
            changed = true;
            id
        };

        if self
            .by_name(owner_name)
            .and_then(|agent| agent.companion.as_deref())
            != Some(reviewer_id.as_str())
        {
            changed |= self
                .set_companion(&owner.id, Some(&reviewer_id))
                .unwrap_or(false);
        }
        if changed {
            let _ = self.save_all();
            self.sort_rail();
        }
        changed
    }

    pub fn ensure_grace_reviewer(&mut self, _llm: &crate::llm::LlmConfig) -> bool {
        self.ensure_tandem_reviewer(
            GRACE,
            PEDANTIC_GRACE_REVIEWER,
            "orchestration-review",
            "Independently reviews Grace's plans, delegation, evidence, and completion claims.",
            GRACE_REVIEWER_ROUTING,
            &crate::llm::default_pedantic_grace_prompt(),
        )
    }

    fn ensure_required_reviewers(&mut self, llm: &crate::llm::LlmConfig) -> usize {
        let mut changed = self.ensure_grace_reviewer(llm) as usize;
        changed += self.ensure_tandem_reviewer(
            FORM_DESIGNER,
            PEDANTIC_FORM_DESIGNER_REVIEWER,
            "form-design-review",
            "Independently reviews form structure, controls, layout, styling, bindings, event delegation, accessibility, and regressions.",
            FORM_DESIGNER_REVIEWER_ROUTING,
            &crate::llm::default_pedantic_ui_prompt(),
        ) as usize;
        changed += self.ensure_tandem_reviewer(
            EVENT_HANDLER,
            PEDANTIC_EVENT_HANDLER_REVIEWER,
            "cobol-event-review",
            "Independently reviews COBOL event handlers for language correctness, exact form integration, behavior, safety, and regressions.",
            EVENT_HANDLER_REVIEWER_ROUTING,
            &crate::llm::default_pedantic_event_prompt(),
        ) as usize;
        changed += self.ensure_tandem_reviewer(
            DOCUMENTATION_AGENT,
            PEDANTIC_DOCUMENTATION_REVIEWER,
            "documentation-review",
            "Independently reviews project documentation for source fidelity, technical accuracy, structure, allowed paths, and indexability.",
            DOCUMENTATION_REVIEWER_ROUTING,
            &crate::llm::default_pedantic_documentation_prompt(),
        ) as usize;
        changed += self.ensure_tandem_reviewer(
            VERSION_CONTROL,
            PEDANTIC_VERSION_CONTROL_REVIEWER,
            "version-control-review",
            "Independently reviews Git operations for scope, repository safety, confirmation, evidence, reversibility, and unintended changes.",
            VERSION_CONTROL_REVIEWER_ROUTING,
            &crate::llm::default_pedantic_version_control_prompt(),
        ) as usize;
        changed += self.ensure_tandem_reviewer(
            DATA_INDEXED_FILE_AGENT,
            PEDANTIC_DATA_INDEXED_FILE_REVIEWER,
            "indexed-file-review",
            "Independently reviews indexed-file schemas, normalization, keys, PIC clauses, helper files, generated artifacts, tool evidence, and regressions.",
            DATA_INDEXED_FILE_REVIEWER_ROUTING,
            &crate::llm::default_pedantic_data_indexed_file_prompt(),
        ) as usize;
        changed
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
        let before = self.agents.len();
        self.ensure_grace();
        self.ensure_form_designer(llm);
        self.ensure_event_handler(llm);
        self.ensure_version_control(llm);
        self.ensure_documentation_agent(llm);
        self.ensure_data_indexed_file_agent(llm);
        self.ensure_required_reviewers(llm);
        self.ensure_builtin_connections(llm);
        self.ensure_specialist_knowledge_tools();
        self.sort_rail();
        self.agents.len().saturating_sub(before)
    }

    /// Ensure the Form Designer specialist exists. Older projects could have
    /// an agent database without this fixed workflow participant.
    pub fn ensure_form_designer(&mut self, llm: &crate::llm::LlmConfig) -> bool {
        if self.by_name(FORM_DESIGNER).is_some() {
            return false;
        }
        match self.create_kinded(
            FORM_DESIGNER,
            &crate::llm::default_form_designer_agent_prompt(),
            AgentKind::Specialist,
            "form-design",
        ) {
            Ok(id) => {
                if let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id) {
                    agent.purpose =
                        "Designs and edits project forms and delegates event implementation."
                            .into();
                    agent.provider = llm.provider.clone();
                    agent.endpoint = llm.endpoint.clone();
                    agent.model = llm.model.clone();
                    agent.temperature = llm.temperature;
                    agent.max_tokens = llm.max_tokens;
                    agent.timeout_secs = llm.timeout_secs;
                    agent.routing = FORM_DESIGNER_ROUTING.into();
                    agent.tools = vec![
                        "egui.tree".into(),
                        "egui.rects".into(),
                    ];
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }

    /// Seed the Form Designer Agent's built-in skill docs (container-carry and
    /// overlap-free moves) into `skills/` and register them in the agent's
    /// `skills` list. Idempotent, and repairs databases created before the
    /// skills existed. User-edited skill files are preserved: content is written
    /// only when the file is absent or empty.
    pub fn ensure_form_designer_skills(&mut self) -> bool {
        let Some(agent) = self.by_name(FORM_DESIGNER) else {
            return false;
        };
        let name = agent.name.clone();
        let skills_dir = self.dir(&name).join("skills");
        let mut changed = false;
        for (file, content) in crate::llm::form_designer_builtin_skills() {
            let path = skills_dir.join(file);
            let missing = std::fs::read_to_string(&path)
                .map(|c| c.trim().is_empty())
                .unwrap_or(true);
            if missing {
                let _ = std::fs::create_dir_all(&skills_dir);
                if std::fs::write(&path, content).is_ok() {
                    changed = true;
                }
            }
            if let Some(a) = self.agents.iter_mut().find(|a| a.name == name) {
                if !a.skills.iter().any(|s| s == file) {
                    a.skills.push(file.to_string());
                    changed = true;
                }
            }
        }
        if changed {
            let _ = self.save_all();
        }
        changed
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
                    v.tools = vec![
                        "git (project repository)".to_string(),
                        "git.run".to_string(),
                    ];
                    v.routing = VERSION_CONTROL_ROUTING.to_string();
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
                    e.routing = EVENT_HANDLER_ROUTING.to_string();
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }

    /// Create or repair the fixed Documentation Agent. Its connection follows
    /// the Form Designer Agent when available, then the project's active model.
    pub fn ensure_documentation_agent(&mut self, llm: &crate::llm::LlmConfig) -> bool {
        if let Some(index) = self
            .agents
            .iter()
            .position(|agent| agent.name.eq_ignore_ascii_case(DOCUMENTATION_AGENT))
        {
            let agent = &mut self.agents[index];
            let required_tools = [
                "documentation.write",
                "documentation.read",
                "documentation.list",
                "knowledge.search",
            ];
            let mut changed = false;
            if agent.kind != AgentKind::Specialist {
                agent.kind = AgentKind::Specialist;
                changed = true;
            }
            if agent.specialization != "documentation" {
                agent.specialization = "documentation".into();
                changed = true;
            }
            for tool in required_tools {
                if !agent.tools.iter().any(|existing| existing == tool) {
                    agent.tools.push(tool.into());
                    changed = true;
                }
            }
            if changed {
                let _ = self.save_all();
            }
            return changed;
        }

        let template = self.by_name(FORM_DESIGNER).cloned();
        let (provider, endpoint, model, temperature, max_tokens, timeout_secs, model_profile) =
            match template {
                Some(agent) => (
                    agent.provider,
                    agent.endpoint,
                    agent.model,
                    agent.temperature,
                    agent.max_tokens,
                    agent.timeout_secs,
                    agent.model_profile,
                ),
                None => (
                    llm.provider.clone(),
                    llm.endpoint.clone(),
                    llm.model.clone(),
                    llm.temperature,
                    llm.max_tokens,
                    llm.timeout_secs,
                    llm.model_profiles
                        .iter()
                        .find(|profile| {
                            profile.provider == llm.provider && profile.model == llm.model
                        })
                        .or_else(|| llm.model_profiles.first())
                        .map(|profile| profile.id.clone()),
                ),
            };
        match self.create_kinded(
            DOCUMENTATION_AGENT,
            &crate::llm::default_documentation_agent_prompt(),
            AgentKind::Specialist,
            "documentation",
        ) {
            Ok(id) => {
                if let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id) {
                    agent.purpose =
                        "Creates and maintains indexed project documentation for Grace and specialists."
                            .into();
                    agent.provider = provider;
                    agent.endpoint = endpoint;
                    agent.model = model;
                    agent.temperature = temperature;
                    agent.max_tokens = max_tokens;
                    agent.timeout_secs = timeout_secs;
                    agent.model_profile = model_profile;
                    agent.routing = DOCUMENTATION_ROUTING.into();
                    agent.tools = vec![
                        "documentation.write".into(),
                        "documentation.read".into(),
                        "documentation.list".into(),
                        "knowledge.search".into(),
                    ];
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }

    /// Create or repair the fixed Data (Indexed File) Agent. The specialist
    /// uses the same `.cidx` model as the Indexed File UI and receives no tools
    /// outside indexed-file maintenance and project-knowledge retrieval.
    pub fn ensure_data_indexed_file_agent(&mut self, llm: &crate::llm::LlmConfig) -> bool {
        let required_tools = [
            "indexed_file.list",
            "indexed_file.read",
            "indexed_file.write",
            "knowledge.search",
        ];
        if let Some(index) = self
            .agents
            .iter()
            .position(|agent| agent.name.eq_ignore_ascii_case(DATA_INDEXED_FILE_AGENT))
        {
            let agent = &mut self.agents[index];
            let mut changed = false;
            if agent.kind != AgentKind::Specialist {
                agent.kind = AgentKind::Specialist;
                changed = true;
            }
            if agent.specialization != "indexed-files" {
                agent.specialization = "indexed-files".into();
                changed = true;
            }
            for tool in required_tools {
                if !agent.tools.iter().any(|existing| existing == tool) {
                    agent.tools.push(tool.into());
                    changed = true;
                }
            }
            if changed {
                let _ = self.save_all();
            }
            return changed;
        }

        let template = self.by_name(FORM_DESIGNER).cloned();
        let (provider, endpoint, model, temperature, max_tokens, timeout_secs, model_profile) =
            match template {
                Some(agent) => (
                    agent.provider,
                    agent.endpoint,
                    agent.model,
                    agent.temperature,
                    agent.max_tokens,
                    agent.timeout_secs,
                    agent.model_profile,
                ),
                None => (
                    llm.provider.clone(),
                    llm.endpoint.clone(),
                    llm.model.clone(),
                    llm.temperature,
                    llm.max_tokens,
                    llm.timeout_secs,
                    llm.model_profiles.first().map(|profile| profile.id.clone()),
                ),
            };
        match self.create_kinded(
            DATA_INDEXED_FILE_AGENT,
            &crate::llm::default_data_indexed_file_agent_prompt(),
            AgentKind::Specialist,
            "indexed-files",
        ) {
            Ok(id) => {
                if let Some(agent) = self.agents.iter_mut().find(|agent| agent.id == id) {
                    agent.purpose = "Creates and maintains normalized project indexed-file definitions through the PowerRustCOBOL Indexed File UI model.".into();
                    agent.provider = provider;
                    agent.endpoint = endpoint;
                    agent.model = model;
                    agent.temperature = temperature;
                    agent.max_tokens = max_tokens;
                    agent.timeout_secs = timeout_secs;
                    agent.model_profile = model_profile;
                    agent.routing = DATA_INDEXED_FILE_ROUTING.into();
                    agent.tools = required_tools.iter().map(|tool| (*tool).into()).collect();
                }
                let _ = self.save_all();
                self.sort_rail();
                true
            }
            Err(_) => false,
        }
    }

    /// Ensure every specialist can retrieve project documentation. Write access
    /// remains exclusive to Documentation Agent and is enforced by the backend.
    pub fn ensure_specialist_knowledge_tools(&mut self) -> usize {
        let mut changed = 0;
        for agent in &mut self.agents {
            if agent.kind == AgentKind::Specialist
                && !agent.tools.iter().any(|tool| tool == "knowledge.search")
            {
                agent.tools.push("knowledge.search".into());
                changed += 1;
            }
        }
        if changed > 0 {
            let _ = self.save_all();
        }
        changed
    }

    fn repair_builtin_definition(
        &mut self,
        name: &str,
        kind: AgentKind,
        specialization: &str,
        purpose: &str,
        routing: &str,
        legacy_routings: &[&str],
        default_prompt: &str,
        required_tools: &[&str],
        llm: &crate::llm::LlmConfig,
    ) -> bool {
        let Some(index) = self
            .agents
            .iter()
            .position(|agent| agent.name.eq_ignore_ascii_case(name))
        else {
            return false;
        };
        let current_prompt = self.load_prompt(name);
        let canonical_prompt = canonicalize_builtin_names(&current_prompt);
        let legacy_form_prompt = name == FORM_DESIGNER
            && (current_prompt.trim() == llm.system_prompt.trim()
                || current_prompt.trim() == crate::llm::DEFAULT_SYSTEM_PROMPT
                || current_prompt.trim() == "You are the PowerRustCOBOL Form Designer Agent.");
        let repaired_prompt = if current_prompt.trim().is_empty()
            || legacy_form_prompt
            || prompt_predates_language_contract(name, &current_prompt)
            || prompt_carries_broken_form_style_guidance(&current_prompt)
        {
            default_prompt.to_string()
        } else {
            canonical_prompt
        };
        let mut changed = false;
        if repaired_prompt != current_prompt && self.save_prompt(name, &repaired_prompt).is_ok() {
            changed = true;
        }

        let agent = &mut self.agents[index];
        if agent.kind != kind {
            agent.kind = kind;
            changed = true;
        }
        if agent.specialization != specialization {
            agent.specialization = specialization.into();
            changed = true;
        }
        if agent.purpose.trim().is_empty() {
            agent.purpose = purpose.into();
            changed = true;
        }
        if agent.routing.trim().is_empty()
            || legacy_routings
                .iter()
                .any(|legacy| agent.routing.trim() == *legacy)
        {
            if agent.routing != routing {
                agent.routing = routing.into();
                changed = true;
            }
        }
        for tool in required_tools {
            if !agent.tools.iter().any(|existing| existing == tool) {
                agent.tools.push((*tool).into());
                changed = true;
            }
        }
        changed
    }

    fn ensure_builtin_definitions(&mut self, llm: &crate::llm::LlmConfig) -> usize {
        let form_prompt = crate::llm::default_form_designer_agent_prompt();
        let event_prompt = crate::llm::default_event_handler_prompt();
        let documentation_prompt = crate::llm::default_documentation_agent_prompt();
        let data_indexed_file_prompt = crate::llm::default_data_indexed_file_agent_prompt();
        let grace_reviewer_prompt = crate::llm::default_pedantic_grace_prompt();
        let form_reviewer_prompt = crate::llm::default_pedantic_ui_prompt();
        let event_reviewer_prompt = crate::llm::default_pedantic_event_prompt();
        let documentation_reviewer_prompt = crate::llm::default_pedantic_documentation_prompt();
        let version_control_reviewer_prompt = crate::llm::default_pedantic_version_control_prompt();
        let data_indexed_file_reviewer_prompt =
            crate::llm::default_pedantic_data_indexed_file_prompt();
        let version_control_prompt = crate::llm::default_version_control_prompt();
        let mut changed = 0;
        changed += self.repair_builtin_definition(
            FORM_DESIGNER,
            AgentKind::Specialist,
            "form-design",
            "Designs and edits project forms and delegates event implementation.",
            FORM_DESIGNER_ROUTING,
            &["Receives: user form requests"],
            &form_prompt,
            &["egui.tree", "egui.rects"],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            EVENT_HANDLER,
            AgentKind::Specialist,
            "cobol-events",
            "Implements delegated COBOL-85 / RustCOBOL event handlers.",
            EVENT_HANDLER_ROUTING,
            &["Receives: delegations from Form Designer Agent"],
            &event_prompt,
            &[],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            DOCUMENTATION_AGENT,
            AgentKind::Specialist,
            "documentation",
            "Creates and maintains indexed project documentation for Grace and specialists.",
            DOCUMENTATION_ROUTING,
            &[
                "Receives every documentation creation or update delegated by Grace.",
                "Receives documentation tasks from Grace after authoritative domain-specialist handoff; writes only under Documentation/ or docs/ through documentation tools; for indexed-file work retrieves project knowledge, establishes file name and purpose, applies 1NF/2NF/3NF, requires the developer's UUID-or-PIC ID choice, and returns schema/helper-file requests to Grace without mutating .cidx files.",
            ],
            &documentation_prompt,
            &[
                "documentation.write",
                "documentation.read",
                "documentation.list",
                "knowledge.search",
            ],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            DATA_INDEXED_FILE_AGENT,
            AgentKind::Specialist,
            "indexed-files",
            "Creates and maintains normalized project indexed-file definitions through the PowerRustCOBOL Indexed File UI model.",
            DATA_INDEXED_FILE_ROUTING,
            &[],
            &data_indexed_file_prompt,
            &[
                "indexed_file.list",
                "indexed_file.read",
                "indexed_file.write",
                "knowledge.search",
            ],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            PEDANTIC_GRACE_REVIEWER,
            AgentKind::Pedantic,
            "orchestration-review",
            "Independently reviews Grace's plans, delegation, evidence, and completion claims.",
            GRACE_REVIEWER_ROUTING,
            &["Reviews: Grace orchestration"],
            &grace_reviewer_prompt,
            &[],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            PEDANTIC_FORM_DESIGNER_REVIEWER,
            AgentKind::Pedantic,
            "form-design-review",
            "Independently reviews form structure, controls, layout, styling, bindings, event delegation, accessibility, and regressions.",
            FORM_DESIGNER_REVIEWER_ROUTING,
            &["Reviews: Form Designer Agent"],
            &form_reviewer_prompt,
            &[],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            PEDANTIC_EVENT_HANDLER_REVIEWER,
            AgentKind::Pedantic,
            "cobol-event-review",
            "Independently reviews COBOL event handlers for language correctness, exact form integration, behavior, safety, and regressions.",
            EVENT_HANDLER_REVIEWER_ROUTING,
            &["Reviews: COBOL Event Handler Script Agent"],
            &event_reviewer_prompt,
            &[],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            PEDANTIC_DOCUMENTATION_REVIEWER,
            AgentKind::Pedantic,
            "documentation-review",
            "Independently reviews project documentation for source fidelity, technical accuracy, structure, allowed paths, and indexability.",
            DOCUMENTATION_REVIEWER_ROUTING,
            &[],
            &documentation_reviewer_prompt,
            &[],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            PEDANTIC_VERSION_CONTROL_REVIEWER,
            AgentKind::Pedantic,
            "version-control-review",
            "Independently reviews Git operations for scope, repository safety, confirmation, evidence, reversibility, and unintended changes.",
            VERSION_CONTROL_REVIEWER_ROUTING,
            &[],
            &version_control_reviewer_prompt,
            &[],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            PEDANTIC_DATA_INDEXED_FILE_REVIEWER,
            AgentKind::Pedantic,
            "indexed-file-review",
            "Independently reviews indexed-file schemas, normalization, keys, PIC clauses, helper files, generated artifacts, tool evidence, and regressions.",
            DATA_INDEXED_FILE_REVIEWER_ROUTING,
            &[],
            &data_indexed_file_reviewer_prompt,
            &[],
            llm,
        ) as usize;
        changed += self.repair_builtin_definition(
            VERSION_CONTROL,
            AgentKind::Specialist,
            "version-control",
            "Manages Git for the project: branches, commits, push, revert, rebase.",
            VERSION_CONTROL_ROUTING,
            &[
                "Delegated by Grace · version-control for the project repo",
                "Delegated by Grace - version-control for the project repo",
            ],
            &version_control_prompt,
            &["git (project repository)", "git.run"],
            llm,
        ) as usize;
        if changed > 0 {
            let _ = self.save_all();
        }
        changed
    }

    fn ensure_builtin_connections(&mut self, llm: &crate::llm::LlmConfig) -> usize {
        let profile_id = llm
            .model_profiles
            .iter()
            .find(|profile| profile.provider == llm.provider && profile.model == llm.model)
            .or_else(|| llm.model_profiles.first())
            .map(|profile| profile.id.clone());
        let mut changed = 0;
        for name in [
            GRACE,
            FORM_DESIGNER,
            EVENT_HANDLER,
            VERSION_CONTROL,
            DOCUMENTATION_AGENT,
            DATA_INDEXED_FILE_AGENT,
        ] {
            let Some(agent) = self
                .agents
                .iter_mut()
                .find(|agent| agent.name.eq_ignore_ascii_case(name))
            else {
                continue;
            };
            // A selected profile, including a dangling one, is an explicit
            // project choice and is never silently replaced here — and neither
            // is an explicit "(no model)", which would otherwise be re-seeded on
            // the next project open and look like it never saved.
            if agent.no_model
                || agent.model_profile.is_some()
                || !agent.provider.trim().is_empty()
                || !agent.model.trim().is_empty()
            {
                continue;
            }
            if let Some(profile_id) = profile_id.clone() {
                agent.model_profile = Some(profile_id);
                changed += 1;
            } else if !llm.provider.trim().is_empty() && !llm.model.trim().is_empty() {
                agent.provider = llm.provider.clone();
                agent.endpoint = llm.endpoint.clone();
                agent.model = llm.model.clone();
                agent.temperature = llm.temperature;
                agent.max_tokens = llm.max_tokens;
                agent.timeout_secs = llm.timeout_secs;
                changed += 1;
            }
        }
        if changed > 0 {
            let _ = self.save_all();
        }
        changed
    }

    /// Seed and repair all agents required by the IDE's built-in workflows.
    /// Idempotent and safe to call on project open and before every Grace run.
    pub fn ensure_fixed_agents(&mut self, llm: &crate::llm::LlmConfig) -> usize {
        let mut changed = self.migrate_builtin_aliases();
        changed += self.seed_from_legacy(llm);
        if self.ensure_grace() {
            changed += 1;
        }
        if self.ensure_form_designer(llm) {
            changed += 1;
        }
        if self.ensure_form_designer_skills() {
            changed += 1;
        }
        if self.ensure_event_handler(llm) {
            changed += 1;
        }
        if self.ensure_version_control(llm) {
            changed += 1;
        }
        if self.ensure_documentation_agent(llm) {
            changed += 1;
        }
        if self.ensure_data_indexed_file_agent(llm) {
            changed += 1;
        }
        changed += self.ensure_required_reviewers(llm);
        changed += self.ensure_builtin_definitions(llm);
        changed += self.ensure_builtin_connections(llm);
        changed += self.ensure_specialist_knowledge_tools();
        self.sort_rail();
        changed
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
    let prompt = db.load_agent_core_instructions(&a.name);
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
            let ped_prompt = db.load_agent_core_instructions(&c.name);
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
    // An explicit "(no model)" wins over everything, including any dormant
    // embedded config left over from before the choice.
    if a.no_model {
        return None;
    }
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
    !a.no_model && (a.model_profile.is_some() || !a.model.trim().is_empty())
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
        if a.no_model || a.model_profile.is_some() {
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
    fn builtin_prompt_migration_redirects_legacy_project_document_paths() {
        let migrated = canonicalize_builtin_names(
            "Read /Documentation/plan.md and /docs/tasks.md with DocumentationAgent.",
        );
        assert_eq!(
            migrated,
            "Read /Knowledge Base/plan.md and /Knowledge Base/tasks.md with Documentation Agent."
        );
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
        assert_eq!(
            llm.model_profiles.len(),
            2,
            "identical configs collapse to 2 profiles"
        );
        for n in ["A", "B", "C"] {
            assert!(
                db.by_name(n).unwrap().model_profile.is_some(),
                "{n} references a profile"
            );
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

    /// Choosing "(no model)" for a built-in agent must survive the next project
    /// open: `ensure_fixed_agents` used to re-seed any built-in whose profile was
    /// unset, so the choice looked like it never saved.
    #[test]
    fn explicit_no_model_survives_builtin_seeding() {
        let proj = tmp_project();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.provider = "anthropic".into();
        llm.model = "claude-sonnet-5".into();
        llm.endpoint = "https://api.anthropic.com/v1/messages".into();
        let mut db = AgentsDb::load(&proj);
        db.ensure_fixed_agents(&llm);

        // Seeding gave the built-in a model; the developer now clears it.
        let grace = db.by_name(GRACE).unwrap().id.clone();
        {
            let a = db.agents.iter_mut().find(|x| x.id == grace).unwrap();
            assert!(a.model_profile.is_some() || !a.model.is_empty());
            a.model_profile = None;
            a.no_model = true;
        }
        db.save_all().unwrap();

        // Re-open the project: the choice is still there, and nothing resolves.
        let mut reopened = AgentsDb::load(&proj);
        reopened.ensure_fixed_agents(&llm);
        let a = reopened.by_name(GRACE).unwrap();
        assert!(a.no_model, "explicit (no model) was re-seeded");
        assert!(a.model_profile.is_none());
        assert!(resolve_agent_connection(a, &llm).is_none());
        assert!(!agent_has_model(a));
        let _ = std::fs::remove_dir_all(proj);
    }

    /// The marker also beats the dormant embedded connection kept for rollback.
    #[test]
    fn explicit_no_model_beats_embedded_fallback() {
        let proj = tmp_project();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        let mut db = AgentsDb::load(&proj);
        let id = db.create("Solo", "p").unwrap();
        set_embedded(&mut db, &id, "prov", "https://e", "m1");
        assert!(resolve_agent_connection(db.by_name("Solo").unwrap(), &llm).is_some());

        db.agents
            .iter_mut()
            .find(|x| x.id == id)
            .unwrap()
            .no_model = true;
        assert!(resolve_agent_connection(db.by_name("Solo").unwrap(), &llm).is_none());
        // …and it is not silently migrated onto a synthesised profile either.
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
        assert!(
            n >= 4,
            "seeded specialists + companions migrated onto profiles"
        );
        let d = db.by_name("Form Designer Agent").unwrap();
        assert!(
            d.model_profile.is_some(),
            "seeded Form Designer references a profile (AC7)"
        );
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
        assert!(
            agent_effective_config(&db, &llm, "A").is_some(),
            "resolves via its profile"
        );
        // Delete the referenced profile → clear "no model" state, not a crash (R8).
        llm.model_profiles.clear();
        assert!(agent_effective_config(&db, &llm, "A").is_none());
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn agent_call_uses_model_id_and_key_from_selected_profile() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        let agent_id = db.create("A", "p").unwrap();
        set_embedded(
            &mut db,
            &agent_id,
            "openai",
            "https://stale.example/v1",
            "stale-model",
        );
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        let profile_id = llm.find_or_create_profile(
            "ollama_cloud",
            "https://ollama.com/api/chat",
            "qwen3.5:397b",
            0.4,
            8192,
            120,
        );
        llm.store_api_key(crate::llm::profile_api_key_slot(&profile_id), "profile-key");
        db.agents
            .iter_mut()
            .find(|agent| agent.id == agent_id)
            .unwrap()
            .model_profile = Some(profile_id);

        let cfg = resolve_agent_connection(db.by_id(&agent_id).unwrap(), &llm).unwrap();
        assert_eq!(cfg.provider, "ollama_cloud");
        assert_eq!(cfg.endpoint, "https://ollama.com/api/chat");
        assert_eq!(cfg.model, "qwen3.5:397b");
        assert_eq!(cfg.api_key, "profile-key");
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn create_scaffolds_the_operator_structure_and_round_trips() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        let id = db
            .create("Form Designer Agent", "line one\nline two\n")
            .unwrap();
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
        assert_eq!(
            db.load_prompt("Form Designer Agent"),
            "line one\nline two\n"
        );
        // manifest never contains a key field
        let json = std::fs::read_to_string(dir.join("agent.json")).unwrap();
        assert!(
            !json.to_lowercase().contains("api_key"),
            "key leaked: {json}"
        );
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
    fn companion_relationship_is_one_to_one_from_either_direction() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        let first = db
            .create_kinded("First Specialist", "p", AgentKind::Specialist, "first")
            .unwrap();
        let second = db
            .create_kinded("Second Specialist", "p", AgentKind::Specialist, "second")
            .unwrap();
        let reviewer = db
            .create_kinded("Pedantic One", "p", AgentKind::Pedantic, "review")
            .unwrap();
        let other_reviewer = db
            .create_kinded("Pedantic Two", "p", AgentKind::Pedantic, "review")
            .unwrap();

        assert!(db.set_companion(&first, Some(&reviewer)).unwrap());
        assert_eq!(db.companion_owner(&reviewer).unwrap().id, first);

        assert!(db.set_companion(&second, Some(&reviewer)).unwrap());
        assert!(db.by_id(&first).unwrap().companion.is_none());
        assert_eq!(db.companion_owner(&reviewer).unwrap().id, second);

        assert!(db.set_companion(&second, Some(&other_reviewer)).unwrap());
        assert!(db.companion_owner(&reviewer).is_none());
        assert_eq!(db.companion_owner(&other_reviewer).unwrap().id, second);

        assert!(db.set_companion(&second, None).unwrap());
        assert!(db.companion_owner(&other_reviewer).is_none());
        assert!(db.set_companion(&reviewer, Some(&other_reviewer)).is_err());
        assert!(db.set_companion(&first, Some(&second)).is_err());

        // Older manifests could contain the same reviewer on multiple owners.
        // Loading repairs that legacy state deterministically (alphabetical
        // specialist order after the orchestrator priority).
        db.agents
            .iter_mut()
            .find(|agent| agent.id == first)
            .unwrap()
            .companion = Some(reviewer.clone());
        db.agents
            .iter_mut()
            .find(|agent| agent.id == second)
            .unwrap()
            .companion = Some(reviewer.clone());
        db.save_all().unwrap();
        let repaired = AgentsDb::load(&proj);
        assert_eq!(repaired.companion_owner(&reviewer).unwrap().id, first);
        assert!(repaired.by_id(&second).unwrap().companion.is_none());
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
        // Six built-in primaries, each immediately paired with its own
        // purpose-specific Pedantic reviewer.
        assert_eq!(db.seed_from_legacy(&llm), 12);
        let grace = db.by_name(GRACE).unwrap();
        assert_eq!(grace.kind, AgentKind::Orchestrator);
        assert!(db.orchestrator_violation().is_none());
        assert!(!db.load_prompt(GRACE).is_empty(), "Grace prompt seeded");
        let grace_reviewer = db.by_name(PEDANTIC_GRACE_REVIEWER).unwrap();
        assert_eq!(grace_reviewer.kind, AgentKind::Pedantic);
        assert_eq!(grace.companion.as_deref(), Some(grace_reviewer.id.as_str()));
        assert!(
            grace_reviewer.model.trim().is_empty(),
            "the developer chooses every reviewer model"
        );
        assert_eq!(
            db.load_prompt(PEDANTIC_GRACE_REVIEWER),
            crate::llm::default_pedantic_grace_prompt()
        );
        // The COBOL Event Handler is a fixed specialist with its prompt loaded.
        let ev = db.by_name(EVENT_HANDLER).unwrap().clone();
        assert_eq!(ev.kind, AgentKind::Specialist);
        assert_eq!(ev.specialization, "cobol-events");
        assert!(!ev.model.trim().is_empty(), "event handler has a model");
        assert!(
            db.load_prompt(EVENT_HANDLER).contains("Event Handler"),
            "event-handler prompt loaded"
        );
        // Its purpose-specific Pedantic reviewer exists and is linked even
        // though no reviewer model has been selected yet.
        let pc = db.by_name(PEDANTIC_EVENT_HANDLER_REVIEWER).unwrap();
        assert_eq!(pc.kind, AgentKind::Pedantic);
        assert_eq!(ev.companion.as_deref(), Some(pc.id.as_str()));
        assert!(pc.model.trim().is_empty());
        assert!(db.pair_rule_violation().is_none());
        assert!(db.ensure_event_handler(&llm) == false, "idempotent");
        // The Version Control (Git) specialist is seeded with its prompt.
        let vc = db.by_name(VERSION_CONTROL).unwrap();
        assert_eq!(vc.kind, AgentKind::Specialist);
        assert_eq!(vc.specialization, "version-control");
        let vc_reviewer = db.by_name(PEDANTIC_VERSION_CONTROL_REVIEWER).unwrap();
        assert_eq!(
            vc.companion.as_deref(),
            Some(vc_reviewer.id.as_str()),
            "Git specialist is paired automatically"
        );
        assert!(
            db.load_prompt(VERSION_CONTROL).contains("Version Control"),
            "version-control prompt loaded"
        );
        assert!(db.ensure_version_control(&llm) == false, "idempotent");
        let designer = db.by_name("Form Designer Agent").unwrap().clone();
        let ped = db.by_name(PEDANTIC_FORM_DESIGNER_REVIEWER).unwrap();
        assert_eq!(designer.model, "claude-sonnet-5");
        assert_eq!(designer.companion.as_deref(), Some(ped.id.as_str()));
        assert!(ped.model.trim().is_empty());
        assert!(db.pair_rule_violation().is_none());
        // spec 030 R2: seeded agents declare the concrete tool names governance
        // recognises — VC gets git.run, the Form Designer gets the egui observers.
        assert!(
            db.by_name(VERSION_CONTROL)
                .unwrap()
                .tools
                .iter()
                .any(|t| t == "git.run"),
            "VC declares git.run"
        );
        assert!(
            designer.tools.iter().any(|t| t == "egui.tree")
                && designer.tools.iter().any(|t| t == "egui.rects"),
            "Form Designer declares the egui observe tools"
        );
        let data_agent = db.by_name(DATA_INDEXED_FILE_AGENT).unwrap();
        assert_eq!(data_agent.specialization, "indexed-files");
        assert!(data_agent
            .tools
            .iter()
            .any(|tool| tool == "indexed_file.write"));
        let data_reviewer = db.by_name(PEDANTIC_DATA_INDEXED_FILE_REVIEWER).unwrap();
        assert_eq!(
            data_agent.companion.as_deref(),
            Some(data_reviewer.id.as_str())
        );
        // Second call is a no-op (agents exist).
        assert_eq!(db.seed_from_legacy(&llm), 0);
        // R8: the designer flow resolves to the DB entry, and the DB
        // companion becomes the reviewer (pedantic handles the check).
        let cfg = designer_agent_config(&db, &llm);
        assert_eq!(cfg.model, "claude-sonnet-5");
        assert!(!cfg.reviewer_configured());
        let unreviewed = unreviewed_primaries(&db);
        for name in [
            FORM_DESIGNER,
            EVENT_HANDLER,
            DOCUMENTATION_AGENT,
            VERSION_CONTROL,
            DATA_INDEXED_FILE_AGENT,
        ] {
            assert!(
                unreviewed.contains(&name.to_string()),
                "{name} remains inactive for review until the developer selects its reviewer model"
            );
        }
        // Remove the companion link: the primary is now unreviewed and the
        // resolved config carries NO reviewer (caller must warn the user).
        db.agents
            .iter_mut()
            .find(|a| a.name == "Form Designer Agent")
            .unwrap()
            .companion = None;
        let cfg = designer_agent_config(&db, &llm);
        assert!(!cfg.reviewer_configured());
        // Kinds are stored: reviewers and Grace are not unreviewed primaries.
        let unreviewed = unreviewed_primaries(&db);
        assert!(unreviewed.contains(&"Form Designer Agent".to_string()));
        // Grace is singleton + reserved.
        assert!(!db.ensure_grace(), "second ensure_grace is a no-op");
        assert!(db.create("grace", "p").is_err(), "name reserved");
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn fixed_agents_include_protected_documentation_with_project_retrieval() {
        let proj = tmp_project();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.provider = "openai".into();
        llm.endpoint = "https://api.openai.com/v1".into();
        llm.model = "gpt-test".into();
        let mut db = AgentsDb::load(&proj);

        assert!(db.ensure_fixed_agents(&llm) > 0);
        let grace = db.by_name(GRACE).unwrap().clone();
        let grace_reviewer = db.by_name(PEDANTIC_GRACE_REVIEWER).unwrap().clone();
        assert_eq!(grace_reviewer.kind, AgentKind::Pedantic);
        assert_eq!(grace_reviewer.specialization, "orchestration-review");
        assert_eq!(grace.companion.as_deref(), Some(grace_reviewer.id.as_str()));
        assert_eq!(
            db.load_prompt(PEDANTIC_GRACE_REVIEWER),
            crate::llm::default_pedantic_grace_prompt()
        );
        assert!(db.delete(&grace_reviewer.id).is_err());
        let documentation = db.by_name(DOCUMENTATION_AGENT).unwrap().clone();
        assert_eq!(documentation.kind, AgentKind::Specialist);
        assert_eq!(documentation.specialization, "documentation");
        for tool in [
            "documentation.write",
            "documentation.read",
            "documentation.list",
            "knowledge.search",
        ] {
            assert!(
                documentation.tools.iter().any(|declared| declared == tool),
                "Documentation Agent declares {tool}"
            );
        }
        assert!(db
            .agents
            .iter()
            .filter(|agent| agent.kind == AgentKind::Specialist)
            .all(|agent| agent.tools.iter().any(|tool| tool == "knowledge.search")));
        assert!(db.delete(&documentation.id).is_err());
        assert!(db.by_name(DOCUMENTATION_AGENT).is_some());
        let data_agent = db.by_name(DATA_INDEXED_FILE_AGENT).unwrap();
        for tool in [
            "indexed_file.list",
            "indexed_file.read",
            "indexed_file.write",
            "knowledge.search",
        ] {
            assert!(
                data_agent.tools.iter().any(|declared| declared == tool),
                "Data (Indexed File) Agent declares {tool}"
            );
        }
        for name in [
            EVENT_HANDLER,
            DOCUMENTATION_AGENT,
            DATA_INDEXED_FILE_AGENT,
            FORM_DESIGNER,
            PEDANTIC_GRACE_REVIEWER,
            PEDANTIC_FORM_DESIGNER_REVIEWER,
            PEDANTIC_EVENT_HANDLER_REVIEWER,
            PEDANTIC_DOCUMENTATION_REVIEWER,
            PEDANTIC_DATA_INDEXED_FILE_REVIEWER,
            PEDANTIC_VERSION_CONTROL_REVIEWER,
            VERSION_CONTROL,
        ] {
            let agent = db.by_name(name).unwrap_or_else(|| panic!("missing {name}"));
            assert!(
                !agent.routing.trim().is_empty(),
                "{name} routing is defined"
            );
            assert!(
                !db.load_prompt(name).trim().is_empty(),
                "{name} prompt is defined"
            );
        }
        for (owner_name, reviewer_name) in [
            (GRACE, PEDANTIC_GRACE_REVIEWER),
            (FORM_DESIGNER, PEDANTIC_FORM_DESIGNER_REVIEWER),
            (EVENT_HANDLER, PEDANTIC_EVENT_HANDLER_REVIEWER),
            (DOCUMENTATION_AGENT, PEDANTIC_DOCUMENTATION_REVIEWER),
            (DATA_INDEXED_FILE_AGENT, PEDANTIC_DATA_INDEXED_FILE_REVIEWER),
            (VERSION_CONTROL, PEDANTIC_VERSION_CONTROL_REVIEWER),
        ] {
            let owner = db.by_name(owner_name).unwrap();
            let reviewer = db.by_name(reviewer_name).unwrap();
            assert_eq!(reviewer_name, format!("{owner_name} Pedantic Reviewer"));
            assert_eq!(reviewer.kind, AgentKind::Pedantic);
            assert_eq!(owner.companion.as_deref(), Some(reviewer.id.as_str()));
            assert!(reviewer.model_profile.is_none());
            assert!(reviewer.provider.trim().is_empty());
            assert!(reviewer.model.trim().is_empty());
            assert!(is_fixed_agent_name(reviewer_name));
        }
        assert!(db
            .load_prompt(FORM_DESIGNER)
            .contains("You are the PowerRustCOBOL Form Designer Agent"));
        db.save_prompt(PEDANTIC_GRACE_REVIEWER, "project-edited reviewer prompt")
            .unwrap();
        assert_eq!(db.ensure_fixed_agents(&llm), 0, "repair is idempotent");
        assert_eq!(
            db.load_prompt(PEDANTIC_GRACE_REVIEWER),
            "project-edited reviewer prompt",
            "fixed-agent repair must preserve project prompt edits"
        );
        let _ = std::fs::remove_dir_all(proj);
    }

    /// Agent prompts are files inside the project, so an existing project keeps
    /// its stale copy until repair rewrites it. A prompt still teaching the
    /// discredited `Theme`/`UPDATE_FORM_PROPERTY` route must be replaced, while
    /// an unrelated project edit is still preserved.
    #[test]
    fn project_open_replaces_prompts_with_broken_form_style_guidance() {
        let proj = tmp_project();
        let llm = crate::llm::LlmConfig::load_defaults_for_test();
        let mut db = AgentsDb::load(&proj);
        db.ensure_fixed_agents(&llm);

        // Simulate the project provisioned by an older release.
        db.save_prompt(
            FORM_DESIGNER,
            "You are the PowerRustCOBOL Form Designer Agent.\n\
             Apply the theme by setting the Form's `Theme` property to the theme \
             ID (e.g. \"neumorphic-dark\") via `UPDATE_FORM_PROPERTY`.",
        )
        .unwrap();
        assert!(db.ensure_fixed_agents(&llm) > 0, "repair must fire");

        let repaired = db.load_prompt(FORM_DESIGNER);
        assert!(
            repaired.contains("GlassStyle") && repaired.contains("\"Neumorphic Dark\""),
            "stale prompt must be replaced with the corrected default"
        );

        // The reviewer's stale prompt carries DIFFERENT broken text than the
        // designer's — it demands the `Theme` route and proof that a change was
        // already applied. Both must be detected, or the reviewer keeps
        // rejecting correct work while the designer looks fixed.
        db.save_prompt(
            PEDANTIC_FORM_DESIGNER_REVIEWER,
            "The theme MUST be applied by setting the Form's \"Theme\" property to a predefined \
             theme name (e.g. \"neumorphic\", \"emerald-glass\", \"cobalt-steel\").\n\
             Require actual inspection results proving each change was applied correctly.",
        )
        .unwrap();
        assert!(db.ensure_fixed_agents(&llm) > 0, "reviewer repair must fire");
        assert!(
            db.load_prompt(PEDANTIC_FORM_DESIGNER_REVIEWER)
                .contains("GlassStyle"),
            "stale reviewer prompt must be replaced too"
        );

        // No shipped default may look stale to the detector, or repair would
        // rewrite every prompt on every project open.
        for (name, default) in [
            (FORM_DESIGNER, crate::llm::DEFAULT_FORM_DESIGNER_AGENT_PROMPT),
            (GRACE, crate::llm::DEFAULT_GRACE_PROMPT),
            (
                PEDANTIC_FORM_DESIGNER_REVIEWER,
                crate::llm::DEFAULT_PEDANTIC_UI_PROMPT,
            ),
        ] {
            assert!(
                !prompt_carries_broken_form_style_guidance(default),
                "{name}'s shipped default must not look broken to the detector"
            );
        }
        assert_eq!(db.ensure_fixed_agents(&llm), 0, "repair is idempotent");

        // An unrelated customization is still the developer's to keep.
        db.save_prompt(FORM_DESIGNER, "Our house style: buttons are 90px wide.")
            .unwrap();
        db.ensure_fixed_agents(&llm);
        assert_eq!(
            db.load_prompt(FORM_DESIGNER),
            "Our house style: buttons are 90px wide.",
            "repair must still preserve genuine project prompt edits"
        );
        let _ = std::fs::remove_dir_all(proj);
    }

    /// The Event Handler prompt shipped before the RustCOBOL language contract
    /// told the agent to "emit COBOL-85 conformant code" without naming a
    /// single verb, intrinsic or level this parser accepts. A project seeded
    /// with that text must be upgraded on open — but only when it is that exact
    /// text, so a developer's own prompt is never overwritten.
    #[test]
    fn project_open_upgrades_the_pre_language_contract_event_prompt() {
        let proj = tmp_project();
        let llm = crate::llm::LlmConfig::load_defaults_for_test();
        let mut db = AgentsDb::load(&proj);
        db.ensure_fixed_agents(&llm);

        db.save_prompt(
            EVENT_HANDLER,
            crate::llm::LEGACY_EVENT_HANDLER_PROMPT_V1,
        )
        .unwrap();
        assert!(db.ensure_fixed_agents(&llm) > 0, "upgrade must fire");

        let upgraded = db.load_prompt(EVENT_HANDLER);
        assert!(
            upgraded.contains(crate::llm::EVENT_HANDLER_LANGUAGE_CONTRACT_MARKER),
            "the stored prompt must carry the language contract"
        );
        // The contract is only useful if it actually names what the toolchain
        // accepts: the verb set, the intrinsic set, and the real source format.
        assert!(upgraded.contains("EVALUATE") && upgraded.contains("UNSTRING"));
        assert!(upgraded.contains("NUMVAL-C") && upgraded.contains("STANDARD-DEVIATION"));
        assert!(
            upgraded.contains("free-form and has NO line-length limit"),
            "the margin rule must state the format actually parsed"
        );
        // The delegation/output/review contract survives the merge.
        assert!(upgraded.contains("generate_event_handler"));
        assert!(upgraded.contains("Pedantic Agent companion"));

        // The reviewer is upgraded on the same open: a reviewer still holding
        // the pre-contract prompt has nothing concrete to check the agent's new
        // contract-compliant code against, and would keep rejecting it.
        db.save_prompt(
            PEDANTIC_EVENT_HANDLER_REVIEWER,
            crate::llm::LEGACY_PEDANTIC_EVENT_PROMPT_V1,
        )
        .unwrap();
        assert!(db.ensure_fixed_agents(&llm) > 0, "reviewer upgrade must fire");
        let reviewer = db.load_prompt(PEDANTIC_EVENT_HANDLER_REVIEWER);
        assert!(
            reviewer.contains(crate::llm::PEDANTIC_EVENT_LANGUAGE_CONTRACT_MARKER),
            "the reviewer must carry the contract checklist"
        );
        assert!(
            reviewer.contains("no line-length limit"),
            "the reviewer must not demand a column-72 margin"
        );

        assert_eq!(db.ensure_fixed_agents(&llm), 0, "upgrade is idempotent");
        for (name, default) in [
            (EVENT_HANDLER, crate::llm::DEFAULT_EVENT_HANDLER_PROMPT),
            (
                PEDANTIC_EVENT_HANDLER_REVIEWER,
                crate::llm::DEFAULT_PEDANTIC_EVENT_PROMPT,
            ),
        ] {
            assert!(
                !prompt_predates_language_contract(name, default),
                "{name}'s shipped default must not look stale to its own detector"
            );
        }

        // A developer's own prompt is theirs, however short.
        db.save_prompt(EVENT_HANDLER, "House rule: no EXEC RUST in handlers.")
            .unwrap();
        db.ensure_fixed_agents(&llm);
        assert_eq!(
            db.load_prompt(EVENT_HANDLER),
            "House rule: no EXEC RUST in handlers.",
            "an edited prompt must never be overwritten by the upgrade"
        );
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn fixed_tandems_exist_before_any_model_is_configured() {
        let proj = tmp_project();
        let mut llm = crate::llm::LlmConfig::load_defaults_for_test();
        llm.provider.clear();
        llm.endpoint.clear();
        llm.model.clear();
        llm.model_profiles.clear();
        llm.reviewer_provider.clear();
        llm.reviewer_endpoint.clear();
        llm.reviewer_model.clear();

        let mut db = AgentsDb::load(&proj);
        assert!(db.ensure_fixed_agents(&llm) > 0);
        assert_eq!(db.agents.len(), 12);
        for (owner_name, reviewer_name) in [
            (GRACE, PEDANTIC_GRACE_REVIEWER),
            (FORM_DESIGNER, PEDANTIC_FORM_DESIGNER_REVIEWER),
            (EVENT_HANDLER, PEDANTIC_EVENT_HANDLER_REVIEWER),
            (DOCUMENTATION_AGENT, PEDANTIC_DOCUMENTATION_REVIEWER),
            (DATA_INDEXED_FILE_AGENT, PEDANTIC_DATA_INDEXED_FILE_REVIEWER),
            (VERSION_CONTROL, PEDANTIC_VERSION_CONTROL_REVIEWER),
        ] {
            let owner = db.by_name(owner_name).unwrap();
            let reviewer = db.by_name(reviewer_name).unwrap();
            assert_eq!(owner.companion.as_deref(), Some(reviewer.id.as_str()));
            assert!(reviewer.model_profile.is_none());
            assert!(reviewer.model.trim().is_empty());
        }

        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn project_open_recreates_a_missing_tandem_and_preserves_reviewer_customization() {
        let proj = tmp_project();
        let llm = crate::llm::LlmConfig::load_defaults_for_test();
        let mut db = AgentsDb::load(&proj);
        assert!(db.ensure_fixed_agents(&llm) > 0);

        let documentation_reviewer_id = db
            .by_name(PEDANTIC_DOCUMENTATION_REVIEWER)
            .unwrap()
            .id
            .clone();
        db.save_prompt(
            PEDANTIC_DOCUMENTATION_REVIEWER,
            "Project-specific documentation review rules.",
        )
        .unwrap();
        let reviewer = db
            .agents
            .iter_mut()
            .find(|agent| agent.id == documentation_reviewer_id)
            .unwrap();
        reviewer.model_profile = Some("developer-selected-reviewer-model".into());
        reviewer.skills.push("project-terminology-check".into());
        db.save_all().unwrap();

        assert_eq!(db.ensure_fixed_agents(&llm), 0);
        let reviewer = db.by_name(PEDANTIC_DOCUMENTATION_REVIEWER).unwrap();
        assert_eq!(
            db.load_prompt(PEDANTIC_DOCUMENTATION_REVIEWER),
            "Project-specific documentation review rules."
        );
        assert_eq!(
            reviewer.model_profile.as_deref(),
            Some("developer-selected-reviewer-model")
        );
        assert!(reviewer
            .skills
            .iter()
            .any(|skill| skill == "project-terminology-check"));

        let missing_id = db
            .by_name(PEDANTIC_EVENT_HANDLER_REVIEWER)
            .unwrap()
            .id
            .clone();
        std::fs::remove_dir_all(db.dir(PEDANTIC_EVENT_HANDLER_REVIEWER)).unwrap();
        let mut reopened = AgentsDb::load(&proj);
        assert!(reopened.by_name(PEDANTIC_EVENT_HANDLER_REVIEWER).is_none());
        assert!(reopened.ensure_fixed_agents(&llm) > 0);
        let owner = reopened.by_name(EVENT_HANDLER).unwrap();
        let recreated = reopened.by_name(PEDANTIC_EVENT_HANDLER_REVIEWER).unwrap();
        assert_ne!(recreated.id, missing_id);
        assert_eq!(owner.companion.as_deref(), Some(recreated.id.as_str()));
        assert!(!reopened
            .load_prompt(PEDANTIC_EVENT_HANDLER_REVIEWER)
            .trim()
            .is_empty());
        assert!(recreated.model_profile.is_none());
        assert!(recreated.model.trim().is_empty());

        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn fixed_agent_aliases_migrate_without_losing_identity_or_configuration() {
        let proj = tmp_project();
        let llm = crate::llm::LlmConfig::load_defaults_for_test();
        let mut db = AgentsDb::load(&proj);
        assert!(db.ensure_grace());
        assert!(db.ensure_form_designer(&llm));
        assert!(db.ensure_event_handler(&llm));

        let reviewer_id = db
            .create_kinded(
                LEGACY_PEDANTIC_GRACE_REVIEWER,
                "Custom Pedantic Grace Reviewer project prompt.",
                AgentKind::Pedantic,
                "orchestration-review",
            )
            .unwrap();
        db.agents
            .iter_mut()
            .find(|agent| agent.id == reviewer_id)
            .unwrap()
            .model_profile = Some("stable-reviewer-profile".into());
        let grace_id = db.by_name(GRACE).unwrap().id.clone();
        db.set_companion(&grace_id, Some(&reviewer_id)).unwrap();
        db.create_kinded(
            LEGACY_ORCHESTRATOR_REVIEWER,
            "obsolete duplicate prompt",
            AgentKind::Pedantic,
            "orchestration-review",
        )
        .unwrap();

        let documentation_id = db
            .create_kinded(
                LEGACY_DOCUMENTATION_AGENT,
                "Custom DocumentationAgent project prompt.",
                AgentKind::Specialist,
                "documentation",
            )
            .unwrap();

        let form_reviewer_id = db
            .create_kinded(
                LEGACY_FORM_DESIGNER_REVIEWER,
                "Custom Pedantic UI Agent project prompt.",
                AgentKind::Pedantic,
                "form-design-review",
            )
            .unwrap();
        let form_id = db.by_name(FORM_DESIGNER).unwrap().id.clone();
        db.set_companion(&form_id, Some(&form_reviewer_id)).unwrap();

        let event_reviewer_id = db
            .create_kinded(
                LEGACY_EVENT_HANDLER_REVIEWER,
                "Custom Pedantic COBOL Companion project prompt.",
                AgentKind::Pedantic,
                "cobol-event-review",
            )
            .unwrap();
        let event_id = db.by_name(EVENT_HANDLER).unwrap().id.clone();
        db.set_companion(&event_id, Some(&event_reviewer_id))
            .unwrap();
        db.save_all().unwrap();

        assert!(db.ensure_fixed_agents(&llm) > 0);
        assert!(db.by_name(LEGACY_PEDANTIC_GRACE_REVIEWER).is_none());
        assert!(db.by_name(LEGACY_ORCHESTRATOR_REVIEWER).is_none());
        assert!(db.by_name(LEGACY_FORM_DESIGNER_REVIEWER).is_none());
        assert!(db.by_name(LEGACY_EVENT_HANDLER_REVIEWER).is_none());
        assert!(db.by_name(LEGACY_DOCUMENTATION_AGENT).is_none());

        let reviewer = db.by_name(PEDANTIC_GRACE_REVIEWER).unwrap();
        assert_eq!(reviewer.id, reviewer_id);
        assert_eq!(
            reviewer.model_profile.as_deref(),
            Some("stable-reviewer-profile")
        );
        assert_eq!(
            db.by_name(GRACE).unwrap().companion.as_deref(),
            Some(reviewer_id.as_str())
        );
        assert_eq!(
            db.load_prompt(PEDANTIC_GRACE_REVIEWER),
            "Custom Grace Pedantic Reviewer project prompt."
        );

        let documentation = db.by_name(DOCUMENTATION_AGENT).unwrap();
        assert_eq!(documentation.id, documentation_id);
        assert_eq!(
            db.load_prompt(DOCUMENTATION_AGENT),
            "Custom Documentation Agent project prompt."
        );
        assert_eq!(
            db.by_name(PEDANTIC_FORM_DESIGNER_REVIEWER).unwrap().id,
            form_reviewer_id
        );
        assert_eq!(
            db.by_name(FORM_DESIGNER).unwrap().companion.as_deref(),
            Some(form_reviewer_id.as_str())
        );
        assert_eq!(
            db.load_prompt(PEDANTIC_FORM_DESIGNER_REVIEWER),
            "Custom Form Designer Agent Pedantic Reviewer project prompt."
        );
        assert_eq!(
            db.by_name(PEDANTIC_EVENT_HANDLER_REVIEWER).unwrap().id,
            event_reviewer_id
        );
        assert_eq!(
            db.by_name(EVENT_HANDLER).unwrap().companion.as_deref(),
            Some(event_reviewer_id.as_str())
        );
        assert_eq!(
            db.load_prompt(PEDANTIC_EVENT_HANDLER_REVIEWER),
            "Custom COBOL Event Handler Script Agent Pedantic Reviewer project prompt."
        );
        assert!(db.dir(PEDANTIC_GRACE_REVIEWER).exists());
        assert!(db.dir(DOCUMENTATION_AGENT).exists());
        assert!(!db.dir(LEGACY_ORCHESTRATOR_REVIEWER).exists());
        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn form_designer_gets_builtin_layout_skills() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        db.ensure_fixed_agents(&crate::llm::LlmConfig::load_defaults_for_test());

        // Both built-in skills are registered on the agent...
        let agent = db.by_name(FORM_DESIGNER).unwrap();
        assert!(
            agent
                .skills
                .iter()
                .any(|s| s == "container-controls-move-as-a-whole.md"),
            "container-carry skill registered"
        );
        assert!(
            agent
                .skills
                .iter()
                .any(|s| s == "overlap-free-control-moves.md"),
            "overlap-free skill registered"
        );

        // ...seeded to disk and reachable through the LLM-call capabilities.
        let caps = db.load_agent_capabilities(FORM_DESIGNER);
        assert!(caps.contains("--- SKILL (container-controls-move-as-a-whole.md) ---"));
        assert!(caps.contains("Container controls move as a whole"));
        assert!(caps.contains("--- SKILL (overlap-free-control-moves.md) ---"));
        assert!(caps.contains("Overlap-free control moves"));

        // Idempotent: a second pass registers nothing new and does not duplicate.
        db.ensure_form_designer_skills();
        let dupes = db
            .by_name(FORM_DESIGNER)
            .unwrap()
            .skills
            .iter()
            .filter(|s| *s == "overlap-free-control-moves.md")
            .count();
        assert_eq!(dupes, 1, "no duplicate skill registration");

        let _ = std::fs::remove_dir_all(proj);
    }

    #[test]
    fn load_agent_instructions_skills_and_knowledge_files() {
        let proj = tmp_project();
        let mut db = AgentsDb::load(&proj);
        let id = db.create("CustomAgent", "Main core prompt.").unwrap();
        let agent_dir = db.dir("CustomAgent");
        std::fs::write(agent_dir.join("steering/custom_steering.md"), "Custom steering content.").unwrap();
        std::fs::write(agent_dir.join("policies.md"), "Custom policy content.").unwrap();
        std::fs::write(agent_dir.join("skills/custom_skill.md"), "Custom skill content.").unwrap();
        std::fs::write(agent_dir.join("knowledge/custom_doc.md"), "Custom knowledge content.").unwrap();

        let agent = db.agents.iter_mut().find(|a| a.id == id).unwrap();
        agent.steering = vec!["custom_steering.md".into()];
        agent.policies = vec!["policies.md".into()];
        agent.skills = vec!["custom_skill.md".into()];
        agent.knowledge = vec!["custom_doc.md".into()];
        db.save_all().unwrap();

        let loaded = AgentsDb::load(&proj);
        let core = loaded.load_agent_core_instructions("CustomAgent");
        assert!(core.contains("Main core prompt."));
        assert!(core.contains("--- STEERING (custom_steering.md) ---"));
        assert!(core.contains("Custom steering content."));
        assert!(core.contains("--- POLICIES (policies.md) ---"));
        assert!(core.contains("Custom policy content."));

        let skills = loaded.load_agent_capabilities("CustomAgent");
        assert!(skills.contains("--- SKILL (custom_skill.md) ---"));
        assert!(skills.contains("Custom skill content."));

        let knowledge = loaded.load_agent_knowledge("CustomAgent");
        assert!(knowledge.contains("--- KNOWLEDGE (custom_doc.md) ---"));
        assert!(knowledge.contains("Custom knowledge content."));

        let _ = std::fs::remove_dir_all(proj);
    }
}
