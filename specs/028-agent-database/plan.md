<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — spec 028 (agent database, Phase 1)

## Design

- **`crates/cobolt-ide/src/agents_db.rs` (new).**
  - `AgentDef` (serde): id, name, purpose, enabled, provider, endpoint,
    model, temperature, max_tokens, timeout_secs, companion: Option<String>
    (agent id), routing: String, steering/policies/skills/tools/knowledge:
    Vec<String>. Prompt text is NOT in the struct — it lives in
    `<name>_prompt.md` and is loaded/saved separately.
  - `AgentsDb { agents: Vec<AgentDef>, root: PathBuf }`:
    `load(project_root)`, `save_agent`, `create(name, template)`,
    `delete(name)`, `prompt_path(name)`, `load_prompt`, `save_prompt`,
    `validate()` (pair rule R5), `seed_from_legacy(&LlmConfig)`.
  - UUID v4 via the existing `rand` dep (hand-rolled formatting, no new
    crate).
  - Folder scaffold on create: steering/, skills/, knowledge/, policies.md,
    mcp.json, agent.json, <name>_prompt.md.
- **`crates/cobolt-ide/src/panels/agents_modal.rs` (new).** egui port of the
  approved mockup: Window (error-modal scaffold pattern — panels partition
  the box, no Resize ratchet), left rail (dimmed rows, nested companions),
  right single-panel collapsible sections, footer validation, New-agent
  inline name prompt, Delete with confirm, per-model key field backed by
  `llm.api_keys`.
- **Settings integration.** `settings_form.rs`: AI section header replaced
  by the summary row + button (legacy connection fields stay below,
  labelled as the connection defaults, until Phase 2 retires them).
  `app.rs`: owns `agents_modal: Option<AgentsModalState>` + `agents_db`,
  loads on project open, saves on OK/Apply.
- **R8 wiring.** `agents_db::designer_agent_config(&AgentsDb, &LlmConfig)
  -> LlmConfig` — overrides provider/endpoint/model/prompt from the "Form
  Designer Agent" entry (+ key from the slot map); designer send path calls
  it.

## Risks

- egui modal self-inflation → use the panel-partition pattern (skill:
  egui-paint-regressions).
- Name→folder on non-ASCII names: accept as-is (macOS/Win/Linux all handle
  UTF-8 names); forbid path separators and leading dots in validation.
