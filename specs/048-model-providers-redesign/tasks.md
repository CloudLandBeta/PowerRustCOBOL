<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Model Providers redesign

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-08-09

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

**Ordering principle.** The data layer and migration land first, fully tested,
while the old UI still works against the old path — so the build is green after
every task. i18n keys land before the surfaces that use them. The two big UI
rebuilds come next, and profile write-paths are removed only once nothing reads
them. `cargo build -p cobolt-ide` and `cargo test -p cobolt-ide` must be green at
the end of every task.

---

## Stage A — data layer and migration

- [x] **T1 — `ProviderConfig`: the new unit of configuration** (R1, R3, R6, R6a, R7)
  - Files: `crates/cobolt-ide/src/llm.rs`
  - Do: add `ProviderConfig { provider, endpoint, endpoint_user_edited }` and
    `LlmConfig::provider_configs: Vec<ProviderConfig>`, **persisted machine-wide
    in `llm_config.json`** (plan §3.3). Add `provider_key_slot(id) ->
    "providerkey::<id>"` (never `provider::<id>` — that collides with the legacy
    `api_key_slot` shape), `provider_config(id)`, `provider_config_mut(id)`,
    `configured_providers()` (a provider with a key, or a keyless provider whose
    endpoint answers — R6a), and a per-provider fetched-model cache
    `models_for(provider)`. Nothing reads these yet.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide` green. New
    unit tests: a default endpoint comes from `PROVIDERS`; an edited endpoint
    round-trips through save/load; `provider_key_slot` never equals any
    `api_key_slot(p, m)` output for the 16 shipped provider ids.

- [x] **T2 — Migration: profiles → providers + agents** (R24, R25, R26, R27, R28, R29)
  - Files: `crates/cobolt-ide/src/agents_db.rs`
  - Do: add `MigrationReport { agents_migrated, providers_created, discarded,
    endpoint_conflicts }` and `migrate_profiles_to_providers(&mut self, &mut
    LlmConfig) -> MigrationReport` implementing plan §3.5 steps 1–4. Re-key
    `api_key_saved_at`, `natively_stored_slots` and `deleted_api_key_slots` — not
    just the in-session secret (keys do not persist today; the timestamps do).
    Skip `no_model` agents. Leave a dangling profile reference unconfigured.
    Not called from anywhere yet.
  - Verify: `cargo test -p cobolt-ide agents_db` green, with the six plan §6
    migration tests: `profiles_migrate_onto_their_agents`,
    `the_newest_key_per_provider_survives`,
    `a_dangling_profile_reference_leaves_the_agent_unconfigured`,
    `an_explicit_no_model_choice_survives_migration`, `migration_is_idempotent`,
    `endpoint_conflicts_are_reported_not_guessed`. Each prints its before/after
    table rather than a bare assertion count. Covers AC20, AC21.

- [x] **T3 — Resolution from the agent's own fields** (R8, R11, R12, R21, R22, R23)
  - Files: `crates/cobolt-ide/src/agents_db.rs`
  - Do: rework `resolve_agent_connection` so the embedded-fields branch is
    primary and draws endpoint + key from the agent's `ProviderConfig`; keep the
    `model_profile` branch reachable only for a not-yet-migrated agent. Drop the
    `model_profile` arm from `agent_has_model`. Add `assigned_models(&self,
    &LlmConfig) -> Vec<(String, String, String)>` for the leaderboard. Leave
    `model_separation` / `model_is_reserved` logic untouched.
  - Verify: `cargo test -p cobolt-ide agents_db` green. New:
    `an_agent_resolves_from_its_own_fields_and_its_providers_endpoint`,
    `two_agents_on_different_providers_resolve_independently` (AC9),
    `a_specialist_on_graces_model_still_clashes_after_the_redesign` (AC17),
    `the_judge_may_share_graces_model_until_a_specialist_arrives` (AC18). Also
    assert an agent's temperature reaches its resolved config (AC12).

- [x] **T4 — Top-level default model without profiles** (R8)
  - Files: `crates/cobolt-ide/src/llm.rs`, `crates/cobolt-ide/src/app.rs`
  - Do: replace `ensure_default_model_from_profiles` with
    `ensure_default_model_from_agents` (Grace first, then any agent with a model,
    then the first model of any configured provider). The direct AI surfaces use
    the top-level `provider`/`model` and would otherwise be orphaned.
  - Verify: `cargo test -p cobolt-ide` green; a new test asserts Grace's model
    becomes the default and that an agent-less project falls back cleanly.

- [x] **T5 — Run migration on project open and report it** (R24, R26, R27)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: call `migrate_profiles_to_providers` once per project open, before the
    first resolution. Push the report to the Output panel — agents migrated,
    providers created, every discarded credential slot named with its provider,
    endpoint conflicts. **No modal, no prompt.**
  - Verify: `cargo test -p cobolt-ide` green. Observable: open a pre-048 project
    and confirm agents keep the same provider/model/temp/tokens/timeout, no
    prompt appears, and the migration lines are in Output (AC19).

## Stage B — leaderboard

- [x] **T6 — Board sourced from assignments and history** (R17, R18, R19)
  - Files: `crates/cobolt-ide/src/leaderboard.rs`
  - Do: retype `ensure_models` to take assigned `(provider, model, endpoint)`
    triples. Replace `prune_unregistered` with `prune_untested_orphans(assigned)`
    which removes an entry only when `runs == 0` **and** it is unassigned. This
    supersedes the 1.61.6 rule — a row with recorded runs is history and must
    never be swept.
  - Verify: `cargo test -p cobolt-ide leaderboard` green. New:
    `a_row_with_runs_is_never_pruned` (the regression guard against 1.61.6's
    rule returning), `an_untested_row_no_agent_uses_is_pruned_and_named`,
    `assigned_models_populate_the_board`. Covers AC15, AC16.

- [x] **T7 — Wire the board to the agents** (R17, R18)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: re-point `sync_leaderboard_models` and `prune_leaderboard_orphans` at
    `AgentsDb::assigned_models`. Keep the Output reporting added in 1.61.6, with
    the wording updated to say untested orphans.
  - Verify: `cargo test -p cobolt-ide` green; opening the board lists every
    assigned model.

- [ ] **T8 — Leaderboard owns proficiency testing** (R15, R20, and plan D5)
      *(partly done: the assignment paths now write the agent's own connection
      and configure the provider; plan D5's clash guard turned out to already
      exist in `assign_model_to_agents`. The "add a model to test" picker (R20)
      is the remaining UI half.)*
  - Files: `crates/cobolt-ide/src/panels/leaderboard_modal.rs`
  - Do: add an "add a model to test" picker (configured provider → model, R20).
    Guard `apply_to_specialists`: when the chosen model is Grace's or the
    Judge's, **block** with the clash message rather than applying to the
    non-clashing subset (plan D5). ✅ *already enforced via `model_is_reserved`*
  - Verify: `cargo test -p cobolt-ide` green; a test asserts the blocked apply
    changes no agent and returns the clash. Observable: a run started here
    records scores against its `(provider, model)` row (AC14).

## Stage C — i18n

- [x] **T9 — All new strings, six languages** (R2, R9, R10, R14, R26)
      *(24 keys added in all six languages. Retiring the old profile strings and
      `models_title` moves into T10, which is what still renders them —
      `no_surface_still_says_models_manager` lands there too.)*
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: add the ~24 `Tr` fields listed in plan §3.4 — provider manager, agents
    table, validation messages, migration and housekeeping lines — filled in
    EN/ES/PT/JA/ZH/FR. Retire the profile-centric strings and the proficiency
    button labels on the two surfaces losing them. Rename `models_title` to
    `providers_title` with the new wording in all six.
  - Verify: `cargo test -p cobolt-ide i18n` green (no empty translations). New
    test `no_surface_still_says_models_manager` asserts the old label is absent
    from all six tables (AC1). Landing this before the UI means the surfaces can
    reference keys that already exist.

## Stage D — user interface

- [ ] **T10 — Model Providers Manager** (R1, R2, R3, R4, R5, R6, R6a, R16)
  - Files: `crates/cobolt-ide/src/panels/models_modal.rs`
  - Do: rebuild as one row per `PROVIDERS` entry, configured ones first: API key,
    endpoint (defaulted, resettable), "Refresh models", a model-count or error
    line, delete-credential. Remove `draft_profile` / `draft_sel` /
    `confirm_delete` and the whole profile CRUD. **Remove
    `ModelsModalAction::run_proficiency` and its button.** A failed fetch shows
    the error and still permits a hand-typed model id (R5).
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-ide` green.
    Observable: title correct in all six languages (AC1); a saved key populates
    the model list (AC2); endpoint edit survives reopen (AC3); unreachable
    listing shows the error and a typed id still works (AC4); a keyless provider
    offers nothing while a reachable Ollama offers its models (AC5).

- [ ] **T11 — The agents table** (R9, R10, R11, R12, R13, R14, R23, R16)
  - Files: `crates/cobolt-ide/src/panels/agents_modal.rs`
  - Do: build the five-column table — **Agents · Models · Temp · Output Tokens ·
    Timeout** — one row per agent including Grace and the Judge, with the
    provider-scope combobox above it. The combobox filters which models the
    Models column offers and **changes no agent's stored provider**. Model
    combobox is searchable (plan D6) and carries a **(no model)** entry. Validate
    the three numeric fields on entry, keeping the prior value on rejection.
    Show a clash badge on any row violating R21, evaluated at pick time and on
    open (plan D7). Remove the profile picker and the proficiency button.
  - Verify: `cargo test -p cobolt-ide` green. New:
    `the_provider_scope_does_not_change_a_stored_agent_provider` (AC8),
    `out_of_range_tuning_values_are_rejected_and_the_prior_value_stands` (AC11).
    Observable: one row per agent with the five columns (AC7); (no model)
    survives reopen (AC10).

- [ ] **T12 — App wiring** (R15, R16)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: update the three modal wiring blocks for the new action shapes; delete
    the `run_proficiency` handling for the providers manager and the agents
    manager, leaving the leaderboard path.
  - Verify: `cargo test -p cobolt-ide` green. New test
    `no_proficiency_control_exists_outside_the_leaderboard` asserts the action
    fields are gone from both action structs (AC13).

## Stage E — retire the profile layer

- [ ] **T13 — `cobolt.toml` stops carrying model config** (R7, D2)
  - Files: `crates/cobolt-ide/src/project_model.rs`
  - Do: mark `ProjectAiSettings.model_profiles` `#[serde(default,
    skip_serializing)]` so old files still migrate but new saves stop writing
    profiles. `apply_to_llm` no longer seeds live profile state; its
    profile-derived `api_key` lookup is replaced by the provider slot.
  - Verify: `cargo test -p cobolt-ide project_model` green; a saved project's
    `cobolt.toml` contains no `model_profiles` block, and an old one still opens.
    Grep the project directory for any API key and find none (AC6).

- [ ] **T14 — Remove profile write paths** (D2)
  - Files: `crates/cobolt-ide/src/llm.rs`
  - Do: delete `add_profile`, `delete_model_profile`, `profile()` and the
    profile-restore branch in the backup path. Keep `ModelProfile` **deserialisable
    for one release** so migration stays repeatable (plan D2); it is never
    written.
  - Verify: `cargo build -p cobolt-ide` + full `cargo test -p cobolt-ide` green;
    no remaining references to the deleted functions.

## Stage F — documentation and release

- [ ] **T15 — Developer's Guide** (R2, R15, R24)
  - Files: `docs/developers-guide-en.md`
  - Do: rewrite the Models Manager / model configuration / proficiency-testing
    sections for provider configuration, the agents table and Leaderboard-only
    testing. Include the migration note and the caveat that one credential per
    provider survives migration. COBOL examples and prose only — never Rust.
    Translations (`-es/-pt/-jp/-cn`) untouched.
  - Verify: the guide has no stale reference to "Models Manager", model profiles
    or a proficiency button outside the leaderboard.

- [ ] **T16 — Correct the versioning steering** (plan §7)
  - Files: `specs/steering/tech.md`
  - Do: change "features bump the **minor** (`y`)" to the operator's standing
    rule — only the operator raises `x` or `y`; every agent-made change, feature
    or fix, bumps `z`. This contradiction is what made spec Q1 necessary.
  - Verify: `tech.md` and the operator's rule now agree.

- [ ] **T17 — Finalize** (all)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump `VERSION` to **`1.61.8`** (settled: only the operator raises `x`/`y`).
    Add the CHANGELOG entry under `### Added`, stating plainly that the
    leaderboard pruning rule **supersedes 1.61.6** and can no longer remove a row
    that has scores. If any `cobolt-compiler` doc table was touched, run
    `cargo run -p cobolt-ide --example build_chunked_kb` and commit the
    regenerated `assets/knowledge/chunked.data`.
  - Verify: `cargo test --workspace --no-fail-fast` — collect **every**
    `test result` line and report the totals, not a failure grep. Then the plan
    §6 manual pass: title in six languages; one key populates models; Grace and a
    specialist on different providers both run; a clash badge appears; the board
    keeps a scored row through reassignment and sweeps an unscored orphan; a
    pre-048 project opens unchanged with migration lines in Output.

---

## Acceptance-criteria coverage

| AC | Task |
|----|------|
| AC1 | T9, T10 |
| AC2 | T10 |
| AC3 | T1, T10 |
| AC4 | T10 |
| AC5 | T1, T10 |
| AC6 | T13 |
| AC7 | T11 |
| AC8 | T11 |
| AC9 | T3 |
| AC10 | T2, T11 |
| AC11 | T11 |
| AC12 | T3 |
| AC13 | T12 |
| AC14 | T8 |
| AC15 | T6, T7 |
| AC16 | T6 |
| AC17 | T3, T11 |
| AC18 | T3 |
| AC19 | T5 |
| AC20 | T2, T5 |
| AC21 | T2 |

## Done criteria

All acceptance criteria in `spec.md` are checked, tests pass, the English guide
is updated, and the change is a **single feature commit on the `features`
branch** (never mixed with a fix), announced on forum f=96 with the `[Noticia]`
prefix. Do **not** commit, push or publish unless the operator asks.
