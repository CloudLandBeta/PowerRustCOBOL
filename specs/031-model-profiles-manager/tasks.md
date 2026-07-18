<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Model Profiles + Models Manager

- **Status:** done (code T1–T10 green; T11 version/CHANGELOG deferred to spec-027 T16)
- **Plan:** ./plan.md   **Date:** 2026-07-18

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. The workspace stays green
after every task. `cobolt-agents` is not modified.

---

- [x] **T1 — `ModelProfile` type + global store** (R1, R7)
  - Files: `crates/cobolt-ide/src/llm.rs`.
  - Do: add `ModelProfile { id, name, provider, endpoint, model, temperature,
    max_tokens, timeout_secs }` (no key field). Add `LlmConfig.model_profiles:
    Vec<ModelProfile>` (`#[serde(default)]`). Helpers: `profile(id) ->
    Option<&ModelProfile>`; `ModelProfile::resolve(&self, base: &LlmConfig) ->
    LlmConfig` (fills provider/endpoint/model/params + key from
    `api_keys[slot(provider, model)]`); `find_or_create_profile(&mut self,
    provider, endpoint, model, temp, max_tokens, timeout) -> String` (returns the
    id of an existing exact-match profile or creates one; name `"<provider> ·
    <model>"`, de-duplicated).
  - Verify: `cargo test -p cobolt-ide llm::` — `ModelProfile` round-trips inside
    `LlmConfig`; `find_or_create_profile` returns the same id for an identical
    tuple, a new id otherwise; `resolve` pulls the key from `api_keys`. **(AC1,
    AC4)**

- [x] **T2 — `AgentDef.model_profile` reference** (R5)
  - Files: `crates/cobolt-ide/src/agents_db.rs`.
  - Do: add `model_profile: Option<String>` to `AgentDef` (`#[serde(default)]`).
    Keep embedded model fields (dormant). No resolution change yet.
  - Verify: `cargo test -p cobolt-ide agents_db::` — an old manifest without the
    field deserialises (defaults `None`); a manifest with it round-trips.

- [x] **T3 — Resolve config via the profile** (R5, R8, R12)
  - Files: `crates/cobolt-ide/src/agents_db.rs`; `crates/cobolt-ide/src/grace_host.rs`.
  - Do: rewrite `agent_effective_config` (and `DbAgentInvoker::config_for`) to
    resolve from `a.model_profile` → `llm.profile(id)` → `resolve(...)`, including
    the pedantic companion's profile (R12). An agent with no/absent profile
    resolves to `None` / a clear "no model" error (R8) — same as an empty model
    today. Embedded fields are no longer read for resolution.
  - Verify: `cargo test -p cobolt-ide` — an agent pointing at a profile resolves
    to that profile's config (+ key); a companion resolves from its own profile;
    an absent profile id → `None`. **(AC6, AC8)**

- [x] **T4 — Migration: synthesise + assign profiles** (R6)
  - Files: `crates/cobolt-ide/src/agents_db.rs`.
  - Do: `migrate_to_profiles(&mut self, llm: &mut LlmConfig) -> usize` — for each
    agent with embedded config and no `model_profile`, `find_or_create_profile`
    from its exact tuple and assign the id; identical configs collapse; idempotent
    (agents already referencing a profile are skipped). Returns count migrated.
  - Verify: `cargo test -p cobolt-ide agents_db::` — given agents with some
    identical embedded configs, migration creates the **minimal** profile set,
    assigns each, and each agent's `agent_effective_config` is **identical pre vs
    post**; a second call migrates 0. **(AC5)**

- [x] **T5 — Seeding assigns a profile** (R10)
  - Files: `crates/cobolt-ide/src/agents_db.rs`.
  - Do: in `seed_from_legacy` / `ensure_*`, ensure a default profile exists
    (synthesised from `llm`'s connection) and set each seeded agent's
    `model_profile` to it, instead of relying on embedded config for resolution.
  - Verify: `cargo test -p cobolt-ide agents_db::seeding_migrates_the_legacy_pair`
    (extended) — seeded specialists reference a profile and resolve to the default
    connection; a serialised seeded `agent.json` contains **no API key**. **(AC4,
    AC7)**

- [x] **T6 — Models Manager modal (CRUD + connection editor)** (R2)
  - Files: `crates/cobolt-ide/src/panels/models_modal.rs` (new); register in
    `crates/cobolt-ide/src/panels/mod.rs`.
  - Do: `ModelsModal` over `&mut LlmConfig`: list profiles; create / rename /
    duplicate / delete (delete warns when agents reference it — R8); per-profile
    connection editor (provider → default endpoint, endpoint, API key into
    `api_keys[slot]`, model id + fetch model list, test connection) moved from
    `agents_modal.rs`; a proficiency button per profile. All strings via `Tr`.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide` — modal
    constructs; CRUD mutates `llm.model_profiles`; delete-guard reports referencing
    agents. **(AC1; UI checked manually in T11)**

- [x] **T7 — Models Manager button in Project settings** (R3)
  - Files: `crates/cobolt-ide/src/panels/settings_form.rs`; `crates/cobolt-ide/src/app.rs`.
  - Do: add a **Models Manager** button beside **Manage agents…**
    (`action.manage_models`); `app.rs` owns/opens the `ModelsModal` and drains its
    action; call `migrate_to_profiles` at the same first-open/seeding site.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide`; button wiring
    reviewed (opens the modal). **(AC2)**

- [x] **T8 — Agent Manager: profile dropdown + proficiency** (R4, R9)
  - Files: `crates/cobolt-ide/src/panels/agents_modal.rs`.
  - Do: replace the per-agent connection block (~655–720) with a `ComboBox`
    listing `llm.model_profiles` by name (sets `a.model_profile`) and the existing
    **Check proficiency** button beside it (resolves from the agent's profile via
    `pending_proficiency` → `run_proficiency`). Remove the now-moved fetch/key/
    model draft state.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide` — the modal
    compiles without the removed fields; proficiency still resolves the agent's
    config. **(AC3, AC8)**

- [x] **T9 — i18n: new Tr keys ×6 languages** (R11)
  - Files: `crates/cobolt-ide/src/i18n.rs`.
  - Do: add every new string (Models Manager title/buttons, profile-dropdown
    label, delete/orphan warnings, errors) as `Tr` fields in all six languages
    (EN/ES/PT/JA/ZH/FR). No hard-coded literals in T6–T8.
  - Verify: `cargo test -p cobolt-ide i18n` — no empty translations; workspace
    builds. **(AC9)**

- [x] **T10 — Docs (English guide)** (spec §6)
  - Files: `docs/developers-guide-en.md`.
  - Do: update the AI-agent section: model profiles (defined once, global), the
    Models Manager (create/edit/test/delete), and picking a model per agent from a
    dropdown; note that keys never enter project files or built binaries (R7).
    English only — translations untouched.
  - Verify: section renders in the doc viewer; `git status` shows only `-en.md`.

- [x] **T11 — Finalize** (all AC)
  - Files: (version/CHANGELOG **deferred to spec-027 T16** — branch convention).
  - Do: full workspace build + test. Re-check AC1–AC9 each map to a passing test
    above. Do **not** bump version / CHANGELOG here (reserved for T16, as specs
    027–030).
  - Verify: `cargo build` (workspace) and `cargo test` (workspace) green. Manual
    checks per plan §6: Models Manager opens beside Agent Manager (AC2); profiles
    persist across restart (AC1); Agent Manager dropdown selects a profile and
    proficiency runs (AC3/AC8); **AC4b** — build a project with a configured
    profile and `grep` the generated `.cbl` + built binary for the key (must not
    appear). Not committed/pushed — awaiting operator.

## Done criteria
All acceptance criteria in spec.md (AC1–AC9 + AC4b) are covered by a task's
verification, the full workspace builds and tests green, the English guide is
updated (translations untouched), and the change stays a single **feature** whose
version/CHANGELOG bump is reserved for the spec-027 T16 merge gate. Do **not**
commit or push unless the operator asks.
