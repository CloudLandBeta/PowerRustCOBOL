<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 031 — Model Profiles + Models Manager

- **Status:** draft → approved
- **Folder:** specs/031-model-profiles-manager/
- **Author:** Claude (agent), for the operator   **Date:** 2026-07-18
- **Depends on:** spec 028 (agent database), spec 029 (Grace), spec 030 (tool
  execution). Continuation of the agent-manager work on the `egui-035` branch.

## 1. Overview

Today every agent (`AgentDef`) **embeds its own model configuration** —
provider, endpoint, model id, temperature, max_tokens, timeout — and the Agent
Manager repeats that entire provider → endpoint → API-key → model block for each
agent. Configuring several agents means entering the same connection details over
and over. This spec decouples the model from the agent: a connection is defined
**once** as a named, reusable **model profile**, stored globally, and each agent
simply **references** a profile. The full model-configuration UI moves into a
dedicated **Models Manager** modal (opened from Project settings, beside the
Agent Manager), and the Agent Manager's per-agent config block is replaced by a
**profile dropdown with the Proficiency-check button beside it**. Existing agents
migrate automatically — their distinct embedded configs are synthesised into
named profiles and assigned, with no re-entry and nothing broken.

## 2. Goals / Non-goals

**Goals**

- Define a model connection once; reuse it across any number of agents and across
  all projects (profiles are global, like the existing API-key store).
- A Models Manager modal to create / edit / rename / duplicate / delete profiles,
  test the connection, fetch the provider's model list, and run a proficiency
  check — all the model plumbing that is scattered per-agent today.
- Reduce the Agent Manager's per-agent model UI to a **single dropdown** (pick a
  profile) plus the **Proficiency-check button** next to it.
- Migrate existing agents with **zero manual work** and **zero behaviour change**
  in their resolved runtime config.

**Non-goals**

- No change to the proficiency-check *algorithm* or the pedantic tandem — only
  where its button lives (spec 026/029 behaviour is preserved).
- No change to how/where secrets are stored beyond associating them with a
  profile; keys stay in the global secret store, never in project files.
- No cloud sync of profiles (consistent with product non-goals).
- Not adding per-agent parameter *overrides* on top of a profile (an agent takes
  the profile as-is; see Open questions Q2).
- No new provider integrations; the provider set is unchanged.

## 3. User stories

- As a developer configuring several agents, I define "Sonnet-5 (Anthropic)"
  once and pick it from a dropdown for each agent, instead of re-typing the
  endpoint, key, and model every time.
- As a developer, I open **Models Manager** from Project settings to add, edit,
  test, or remove a model profile in one place.
- As a developer creating an agent, I choose its model from a dropdown and click
  **Check proficiency** right beside it, without opening a connection form.
- As an existing user, I upgrade and everything keeps working — my agents already
  point at profiles the app created from their old settings.

## 4. Requirements (EARS)

**Model profiles & global store**

- **R1 (ubiquitous):** The system shall represent a **model profile** as a
  named, reusable bundle of connection configuration: display name, provider,
  endpoint, model id, temperature, max_tokens, timeout (and its API key held in
  the secret store). Profiles are stored **globally** (alongside the existing
  `LlmConfig` store), reusable across all projects and agents.
- **R7 (constraint):** The system shall keep API keys in the global secret store,
  associated with the profile; it shall **not** write any key or secret into an
  agent's `agent.json` or any project file. Model API keys are IDE-only
  configuration and shall **never** be copied into the RAD-generated COBOL, the
  compiled application binary, or a packaged app (`rcrun build` / `package`); a
  built/shipped artifact must contain no model key or secret.

**Models Manager modal**

- **R2 (ubiquitous):** The system shall provide a **Models Manager** modal to
  create, edit, rename, duplicate, and delete model profiles, and — per profile —
  test the connection, fetch the provider's model list, and run the COBOL
  proficiency check.
- **R3 (event):** When the user clicks a **Models Manager** button in Project
  settings (beside the Agent Manager button), the system shall open the Models
  Manager modal.
- **R8 (constraint):** The system shall guard profile deletion: deleting a
  profile that agents reference shall warn the user; an agent whose referenced
  profile no longer exists shall present a clear **"no model configured"** state
  (as an unconfigured agent does today), never a crash or a fabricated config.

**Agent Manager: reference, don't embed**

- **R4 (ubiquitous):** In the Agent Manager, the per-agent provider / endpoint /
  API-key / model configuration block shall be replaced by a **dropdown that
  selects an existing model profile**, with the **Proficiency-check button beside
  it**. The verbose connection form shall no longer appear per agent.
- **R5 (state):** While an agent references a profile, the system shall resolve
  the agent's effective runtime config (provider, endpoint, model, params, key)
  from that profile at invocation time.
- **R9 (event):** When the user clicks the Proficiency button beside an agent's
  model dropdown, the system shall run the proficiency check for that agent's
  **resolved profile** (reviewed by its pedantic companion when set) — the
  existing behaviour, relocated.
- **R12 (state):** While a pedantic companion is set, the system shall resolve
  the companion's model from the companion agent's own referenced profile
  (reviewer resolution keeps working).

**Migration & seeding**

- **R6 (event):** When the agent database is loaded, the system shall
  auto-synthesise model profiles from agents' distinct embedded configs
  (identical configs collapse to one profile), assign each agent the matching
  profile, and preserve each agent's previously-resolved effective config
  exactly. Migration requires no user input and runs once (idempotent).
- **R10 (event):** When the fixed specialists are seeded (spec 028/029), the
  system shall assign them a model profile (synthesised from the default
  connection) rather than embedding raw config.

**Cross-cutting**

- **R11 (constraint):** Every new user-facing string (Models Manager, dropdown,
  buttons, warnings, errors) shall be a `Tr` field translated in all six
  languages (EN/ES/PT/JA/ZH/FR); no hard-coded literals.

## 5. Acceptance criteria

- [ ] **AC1 (R1/R2):** A model profile can be created, edited, renamed,
  duplicated, and deleted in the Models Manager and persists in the global store
  across an app restart (test on the store; UI verified manually).
- [ ] **AC2 (R3):** A **Models Manager** button appears beside the Agent Manager
  button in Project settings and opens the modal.
- [ ] **AC3 (R4):** The Agent Manager detail shows a model-profile dropdown
  listing all profiles plus a Proficiency button beside it; selecting a profile
  sets the agent's reference; the old per-agent connection block is gone.
- [ ] **AC4 (R5/R7):** An agent's resolved runtime config equals its referenced
  profile's config (provider/endpoint/model/params + key from the secret store),
  and no key is present in the agent's `agent.json` (test).
- [ ] **AC4b (R7):** A generated `.cbl` and a compiled/packaged application
  artifact contain no model API key or secret — verified by building a project
  with a configured profile and grepping the generated source + output binary for
  the key (must not appear).
- [ ] **AC5 (R6):** Given agents with embedded configs (some identical), loading
  synthesises the **minimal** set of profiles, assigns each agent, collapses
  duplicates, and every agent resolves to the **same effective config as before**;
  a second load is a no-op (test).
- [ ] **AC6 (R8):** Deleting a profile referenced by an agent leaves that agent
  in a clear "no model configured" state and does not crash (test).
- [ ] **AC7 (R10):** Freshly seeded specialists reference a profile (not embedded
  config) and resolve to the default connection (test).
- [ ] **AC8 (R9/R12):** The proficiency check invoked beside the dropdown runs the
  agent's resolved profile and, when a companion is set, the companion's resolved
  profile — same result path as before (test on resolution; run verified
  manually).
- [ ] **AC9 (R11):** New strings exist in all six `i18n.rs` tables; `cargo test -p
  cobolt-ide` passes and the workspace builds.

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — Models Manager labels/buttons, the profile
  dropdown, delete/orphan warnings, and errors are new `Tr` fields ×6 (R11/AC9).
- **Generated-code / regenerate contract:** No impact — this is IDE
  configuration only; codegen and the regenerate-on-action contract are untouched.
- **Docs (English guide):** The AI-agent section of `docs/developers-guide-en.md`
  needs updating: model profiles, the Models Manager, and picking a model per
  agent from a dropdown. English only — translations user-maintained.
- **Fix vs feature:** **Feature.** Per the established `egui-035` branch
  convention (specs 027–030), the version bump + a single reconciled CHANGELOG
  entry are **reserved for the spec-027 T16 merge gate** — no per-spec bump here.
- **Branch:** Continues on `egui-035` (agent-owned).
- **Data model / compat:** `AgentDef` gains a profile reference; embedded model
  fields become migration-only. Old manifests must load and migrate (R6);
  profiles persist in the global store, not project files (R7).

## 7. Open questions

- **Q1 — Profile identity:** UUID id + unique display name (like agents), or
  identity by unique name alone? Recommend **UUID id + display name** (rename
  without breaking references). Resolve in `/plan`.
- **Q2 — Parameters in the profile vs per-agent overrides:** temperature /
  max_tokens / timeout move **into** the profile (an agent takes the profile
  as-is). Confirm no per-agent override is wanted (recommended: none, matching
  "define once"). Resolve in `/plan`.
- **Q3 — Embedded fields after migration:** remove the now-unused embedded model
  fields from `agent.json`, or leave them dormant for one release? Recommend
  **leave dormant** (safe rollback), stop reading them for resolution. `/plan`.
- **Q4 — Auto-synthesised profile names:** naming scheme for migrated profiles
  (e.g. `provider:model`, deduplicated). Recommend `"<provider> · <model>"`.
  `/plan`.
- **Q5 — Store location:** a new `model_profiles.json` beside `llm_config.json`,
  or a field inside `LlmConfig`. Design detail for `/plan` (both satisfy R1).
