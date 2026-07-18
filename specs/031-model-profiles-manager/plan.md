<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Model Profiles + Models Manager

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-07-18

## 1. Approach

Two resolution points turn an agent into a runnable config today —
`agents_db::agent_effective_config` and `grace_host::DbAgentInvoker::config_for`.
Both read the agent's **embedded** `provider/endpoint/model/temperature/
max_tokens/timeout_secs` and pull the key from `llm.api_keys[slot(provider,
model)]`. This plan inserts a **named model profile** between the agent and its
config: the agent stores a profile *reference*, and both resolution points read
the profile instead. The per-`(provider, model)` key store is reused unchanged,
so a profile carries **no secret of its own** — its key is whatever
`api_keys[slot(profile.provider, profile.model)]` already holds (R7).

- **Profiles are global (R1).** A `ModelProfile { id, name, provider, endpoint,
  model, temperature, max_tokens, timeout_secs }` lives in a new
  `LlmConfig.model_profiles: Vec<ModelProfile>` — same global `llm_config.json`
  the keys already sit in, so there is one load/save path and profiles are
  reusable across every project (resolves Q5). No key field on the profile
  (resolves R7): the key resolves from `api_keys` by `(provider, model)`.
- **Agents reference a profile (R4/R5).** `AgentDef` gains `model_profile:
  Option<String>` (a profile `id`). Resolution reads the referenced profile;
  the embedded model fields stay on `AgentDef` but become **dormant**
  (migration-only, no longer read for resolution — resolves Q3, safe rollback).
- **Models Manager modal (R2/R3).** A new `panels/models_modal.rs` owns the full
  connection UI (provider → endpoint → API key → model id, fetch model list,
  test connection, proficiency) plus profile CRUD (create / rename / duplicate /
  delete). It is opened by a new **Models Manager** button beside **Manage
  agents…** in `settings_form.rs` (`action.manage_models`). This is largely the
  connection block *moved out of* `agents_modal.rs`, not new UI invented.
- **Agent Manager slims to a dropdown (R4/R9).** The per-agent connection block
  (`agents_modal.rs` ~655–720) is replaced by a `ComboBox` listing
  `llm.model_profiles` by name (sets `a.model_profile`) with the existing
  **Check proficiency** button beside it (reuses `pending_proficiency` →
  `run_proficiency`, resolving from the agent's profile).
- **Migration & seeding (R6/R10).** A new `AgentsDb::migrate_to_profiles(&mut
  self, llm: &mut LlmConfig)` runs where both stores are available (alongside
  `seed_from_legacy`, which already takes `&llm`): for each agent with embedded
  config and no `model_profile`, find-or-create a global profile matching its
  exact `(provider, endpoint, model, temperature, max_tokens, timeout_secs)`
  tuple (identical configs collapse to one), assign its id. Idempotent — agents
  that already reference a profile are skipped. Seeding assigns the synthesised
  default profile instead of embedding config.
- **Orphan safety (R8).** A profile id that resolves to nothing makes the agent
  "no model configured" — the same state as an empty model today
  (`agent_effective_config` returns `None`, the caller warns). Deleting a
  referenced profile warns first.
- **Compiled artifacts carry no key (R7 / AC4b).** Model config is IDE-only:
  `cobolt-codegen` / `cobolt-compiler` never receive an `LlmConfig`, so keys
  cannot reach the generated `.cbl` or the built binary. This is an invariant to
  assert and verify by build+grep, not new code.

## 2. Affected crates / files

- `crates/cobolt-ide/src/llm.rs` — `ModelProfile` struct;
  `LlmConfig.model_profiles` field (serde default empty); helpers:
  `profile(id) -> Option<&ModelProfile>`, `find_or_create_profile(&mut self,
  cfg-tuple) -> id` (dedup), `ModelProfile::resolve(&self, base: &LlmConfig) ->
  LlmConfig` (fills provider/endpoint/model/params + key from `api_keys`).
- `crates/cobolt-ide/src/agents_db.rs` — `AgentDef.model_profile:
  Option<String>` (serde default `None`); rewrite `agent_effective_config`
  (and any sibling resolver) to resolve via the profile; `migrate_to_profiles`;
  seeding (`seed_from_legacy` / `ensure_*`) assigns a profile.
- `crates/cobolt-ide/src/grace_host.rs` — `DbAgentInvoker::config_for` resolves
  via the profile (same lookup as `agent_effective_config`).
- `crates/cobolt-ide/src/panels/models_modal.rs` — **new.** Models Manager:
  profile list + CRUD, connection editor (moved from `agents_modal.rs`), model
  fetch, test connection, proficiency.
- `crates/cobolt-ide/src/panels/agents_modal.rs` — replace the connection block
  with the profile dropdown + proficiency button; drop the moved fetch/key/model
  state that now lives in the Models Manager.
- `crates/cobolt-ide/src/panels/settings_form.rs` — **Models Manager** button
  beside Manage agents (`action.manage_models`).
- `crates/cobolt-ide/src/app.rs` — own/open the `ModelsModal`; handle
  `manage_models`; invoke `migrate_to_profiles` at the seeding/first-open site;
  keep the proficiency-run wiring.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields ×6 (Models Manager title,
  buttons, dropdown label, orphan/delete warnings, errors).
- `docs/developers-guide-en.md` — AI-agent section: model profiles, the Models
  Manager, picking a model per agent from a dropdown (English only).

## 3. Data / model changes

- **`ModelProfile`** (new, in `llm.rs`): `{ id: String (uuid v4), name: String,
  provider, endpoint, model: String, temperature: f32, max_tokens: u32,
  timeout_secs: u32 }`. No key field (key stays in `api_keys`).
- **`LlmConfig`**: `+ model_profiles: Vec<ModelProfile>` (`#[serde(default)]` →
  old configs load). Same `llm_config.json`, same `load`/`save`.
- **`AgentDef`**: `+ model_profile: Option<String>` (`#[serde(default)]` → old
  manifests load). Embedded `provider/endpoint/model/temperature/max_tokens/
  timeout_secs` retained but no longer read for resolution (dormant).
- **Migration**: `migrate_to_profiles` is additive and idempotent; old projects
  gain profile references on first open; no on-disk format is broken.
- **Compat**: an app without this change still reads the files (extra fields
  ignored / defaulted).

## 4. Key decisions & alternatives

- **Decision:** Reuse the existing per-`(provider, model)` `api_keys` store for a
  profile's key; no key field on `ModelProfile`. — **Why:** keys are already
  global and shared; avoids duplicating a secret and keeps R7 trivially true
  (nothing new to keep out of project files/binaries). — **Rejected:** a `key`
  field on the profile (duplicates the secret, widens the R7 surface).
- **Decision:** Store profiles inside `LlmConfig` (`llm_config.json`). — **Why:**
  one global store, one load/save path, co-located with the keys they pair with.
  — **Rejected:** a separate `model_profiles.json` (extra IO, no benefit).
- **Decision:** Profile identity = UUID `id` + unique display `name` (resolves
  Q1). — **Why:** rename without breaking agent references; mirrors how agents
  already use id+name. — **Rejected:** name-only identity (rename breaks refs).
- **Decision:** Parameters live in the profile; agents take it wholesale
  (resolves Q2). — **Why:** matches "define once"; simplest mental model. —
  **Rejected:** per-agent overrides (re-introduces the duplication we're removing;
  can be a later spec if needed).
- **Decision:** Keep embedded `AgentDef` model fields dormant after migration
  (resolves Q3). — **Why:** safe rollback; old app still reads them. —
  **Rejected:** deleting them now (irreversible; risky mid-branch).
- **Decision:** Migrate via `migrate_to_profiles(&mut self, &mut llm)` called with
  seeding. — **Why:** synthesising *global* profiles needs `LlmConfig`, which
  `AgentsDb::load` doesn't have. — **Rejected:** migrating inside `load` (no
  access to the global store).
- **Decision:** Migrated profile name = `"<provider> · <model>"`, de-duplicated
  with a numeric suffix on name collision (resolves Q4).

## 5. Risks & mitigations

- **Risk:** migration maps an agent to the wrong/merged config. → **Mitigation:**
  find-or-create matches the *exact* config tuple; a test asserts each agent's
  resolved effective config is byte-identical before vs after migration (AC5).
- **Risk:** two profiles sharing `(provider, model)` share one key. →
  **Mitigation:** by design (keys are per provider+model); documented; the Models
  Manager shows the resolved key per profile so it's visible.
- **Risk:** deleting a profile silently breaks agents. → **Mitigation:** delete
  warns; orphaned agents resolve to `None` ("no model configured"), never a crash
  (R8/AC6 test).
- **Risk:** reviewer/companion resolution regresses. → **Mitigation:** companion
  resolves through the same profile path; a test covers a primary+companion pair
  resolving both profiles (AC8).
- **Risk:** a key leaks into a built artifact. → **Mitigation:** codegen/compiler
  never take `LlmConfig`; AC4b build+grep proves the generated `.cbl` and binary
  contain no key.
- **Risk:** version-bump collision on the branch. → **Mitigation:** none here —
  deferred to spec-027 T16 (branch convention).

## 6. Test strategy

Unit/integration (add; each asserts + reports real values):

- **`llm.rs`:** `ModelProfile` round-trips inside `LlmConfig`;
  `find_or_create_profile` returns the same id for an identical tuple and a new
  id for a different one; `ModelProfile::resolve` fills provider/endpoint/model/
  params and pulls the key from `api_keys`. (AC1, AC4)
- **`agents_db.rs`:**
  - migration synthesises the **minimal** profile set from agents with some
    identical configs, assigns each agent, and every agent's
    `agent_effective_config` is **unchanged pre vs post**; a second call is a
    no-op. (AC5)
  - a deleted/absent profile id → `agent_effective_config` returns `None`
    (no-model state), no panic. (AC6)
  - a freshly seeded specialist references a profile and resolves to the default
    connection. (AC7)
  - a serialised migrated `agent.json` contains **no API key**. (AC4)
  - a primary+companion pair resolves both models from their profiles. (AC8)
- **i18n:** the existing "all six languages present" test covers the new fields;
  `cargo test -p cobolt-ide` green, workspace builds. (AC9)

Manual / visual (operator-run — I do not drive the app):

- Project settings shows **Models Manager** beside **Manage agents…**; it opens
  (AC2). Create/edit/delete a profile; it persists across restart (AC1).
- Agent Manager shows a profile **dropdown** + **Check proficiency** beside it;
  selecting a profile sets the agent's model; proficiency runs (AC3/AC8).
- **AC4b:** build a project with a configured profile; `grep` the generated
  `.cbl` and the built binary for the key string — it must not appear.

## 7. Steering compliance

- [ ] i18n: all new UI strings in 6 languages.
- [x] Generated-code banner + regenerate-on-action contract preserved — no
      codegen change; R7 further guarantees no key reaches generated code/binary.
- [ ] English dev guide updated (translations untouched).
- [ ] Fix vs feature: **feature** → version/CHANGELOG **deferred to spec-027
      T16** (branch convention, as specs 027–030).
- [x] No "cobolt" in user-facing text; COBOL identifiers/source stay English.
