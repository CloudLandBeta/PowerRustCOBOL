<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Model Providers redesign

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-08-09

## 1. Approach

The design removes a layer rather than adding one. Today three things stack up:
a **provider** (a `PROVIDERS` entry with a default endpoint), a **profile**
(`ModelProfile`: provider + endpoint + model + three tuning values + a key), and
an **agent** that references a profile by id. The middle layer is the one being
retired.

Afterwards there are two layers:

- **`ProviderConfig`** — one per provider actually configured: endpoint plus a
  credential slot. This satisfies R1, R3 and R6/R6a.
- **`AgentDef`** — already carries `provider`, `endpoint`, `model`,
  `temperature`, `max_tokens`, `timeout_secs`. Spec 031 made these dormant
  behind `model_profile`; this plan makes them authoritative again (R8, R12).

The single most useful fact about this change is that **`resolve_agent_connection`
already contains the target code path.** Its third branch resolves an agent from
embedded fields for "un-migrated" agents. The work is to promote that branch to
primary, have it draw endpoint and key from `ProviderConfig` instead of the
legacy `provider::model` slot, and demote the profile branch to migration-only
(R24).

Model availability (R4) needs no new networking: `llm::spawn_list_models` already
reads an OpenAI-wire `/models` listing and `model_list_url` already special-cases
Groq, the HuggingFace router, Alibaba and Ollama. What changes is *what it is
keyed by* — a provider, not a profile — and *where the result is cached* — a
per-provider list held on `LlmConfig`, offered to every agent row (R10).

Proficiency testing (R15/R16) is a deletion: drop `ModelsModalAction::run_proficiency`
and `AgentsAction::run_proficiency` along with the buttons that set them, leaving
`LeaderboardAction::run_tests`, which already exists and already keys on
`(provider, model)`.

The separation rule (R21–R23) is untouched in meaning. `model_separation()`
computes from `agent_model_key()`, which calls `resolve_agent_connection()` — so
it keeps working through the resolution change with no edit to its logic. Only
its *call site* moves: it must now be evaluated as the developer picks a model in
the agents table.

Leaderboard sourcing (R17–R19) is where behaviour genuinely changes, and it
revises the 1.61.6 fix shipped two commits ago. `ensure_models(profiles)` and
`prune_unregistered(profiles)` both take `&[ModelProfile]`. They become:

- `ensure_models(assigned)` — `assigned` being the `(provider, model, endpoint)`
  triples currently held by agents;
- `prune_untested_orphans(assigned)` — removes an entry only when `runs == 0`
  **and** it is not in `assigned` (R18/R19). An entry with recorded runs is
  history and survives regardless.

## 2. Affected crates / files

Everything is in `cobolt-ide`; no other crate is touched.

| File | Change |
|------|--------|
| `crates/cobolt-ide/src/llm.rs` | Add `ProviderConfig` + `provider_configs: Vec<ProviderConfig>`, `provider_config(id)`, `provider_key_slot(id)`, `configured_providers()`, `models_for(provider)` cache. Rework `spawn_list_models` callers to key by provider. Replace `ensure_default_model_from_profiles` with `ensure_default_model_from_agents`. Keep `ModelProfile` **deserialisable only** (Q2) and delete `profile()`, `add_profile`, `delete_model_profile`, `ensure_default_model_from_profiles` write paths. |
| `crates/cobolt-ide/src/agents_db.rs` | `resolve_agent_connection`: embedded fields become primary, endpoint/key from `ProviderConfig`; profile branch retained only for the migration pass. Add `migrate_profiles_to_providers(&mut LlmConfig) -> MigrationReport`. Add `assigned_models(&self, &LlmConfig) -> Vec<(String,String,String)>` for the leaderboard. `agent_has_model` drops its `model_profile` arm. `model_separation` / `model_is_reserved` unchanged. |
| `crates/cobolt-ide/src/panels/models_modal.rs` | Rebuilt as the **Model Providers Manager**: one row per `PROVIDERS` entry (configured ones first), each with API key, endpoint (defaulted + resettable), "Refresh models", a model count / error line, and Delete-credential. Remove the proficiency button and `run_proficiency` from `ModelsModalAction`. Remove `draft_profile` / `draft_sel` / `confirm_delete` profile machinery. |
| `crates/cobolt-ide/src/panels/agents_modal.rs` | Add the five-column table (R9) with the provider-scope combobox (R10). Per-row: model combobox (filtered to the scoped provider, searchable), temp / max-tokens / timeout fields with validation (R14), and a **(no model)** entry (R13). Show a clash badge on rows violating R21. Remove the proficiency button and the profile picker. |
| `crates/cobolt-ide/src/panels/leaderboard_modal.rs` | Add "add a model to test" (provider + model picker, R20). Guard `apply_to_specialists` against R21 (Q5). Everything else unchanged. |
| `crates/cobolt-ide/src/leaderboard.rs` | `ensure_models` retyped to assigned triples; `prune_unregistered` → `prune_untested_orphans` with the zero-runs condition (R18/R19). |
| `crates/cobolt-ide/src/app.rs` | Run migration once on project open and report to Output (R24–R27). Re-point `sync_leaderboard_models` / `prune_leaderboard_orphans` at `assigned_models`. Update the three modal wiring blocks. Drop the `run_proficiency` handling for two of the three surfaces. |
| `crates/cobolt-ide/src/project_model.rs` | `ProjectAiSettings.model_profiles` becomes read-only legacy (`#[serde(default, skip_serializing)]`) so `cobolt.toml` stops growing profiles but old files still migrate. `apply_to_llm` stops seeding `llm.model_profiles` as live state. |
| `crates/cobolt-ide/src/i18n.rs` | New `Tr` fields ×6 languages (§3.4); retire profile-only strings. |
| `docs/developers-guide-en.md` | Rewrite the Models Manager / model configuration / proficiency sections for providers, the agents table and Leaderboard-only testing, with a migration note. Translations untouched. |
| `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` | Feature entry + version bump (see §7). |

## 3. Data / model changes

### 3.1 New type

```rust
pub struct ProviderConfig {
    pub provider: String,            // a PROVIDERS id
    pub endpoint: String,            // defaults to PROVIDERS[].default_endpoint
    pub endpoint_user_edited: bool,  // same meaning as today
}
```

Held as `LlmConfig::provider_configs: Vec<ProviderConfig>`. The credential is
**not** a field — it stays in the existing `api_keys` map under a new slot.

### 3.2 Credential slots

A third slot shape joins the two that exist:

| Shape | Meaning |
|-------|---------|
| `<provider>::<model>` | legacy, pre-031 |
| `profile::<uuid>` | spec 031, being retired |
| `providerkey::<provider-id>` | **new** |

The prefix is deliberately *not* `provider::<id>`: that is indistinguishable from
`api_key_slot("provider", id)` and would collide with the legacy shape.

**What actually persists matters here.** `LlmConfig::api_keys` is
`#[serde(skip)]`, and `crate::secrets` (the OS vault) is suspended until 1.90.0 —
so **keys do not survive a restart today**. What *does* persist is
`api_key_saved_at` (slot → unix seconds), `natively_stored_slots`, and
`deleted_api_key_slots`. Migration therefore re-keys those three maps, not just
the in-session secrets, or a migrated project would lose its "this key is four
months old" 401 diagnostics.

### 3.3 Where provider configuration lives

**Decision: machine-wide, in `llm_config.json`, beside the credentials.** See
§4 D1 for the reasoning and the rejected alternative. `cobolt.toml`'s
`[ai].model_profiles` becomes read-only legacy input for the migration and is no
longer written.

### 3.4 New i18n keys (all six languages)

Provider manager: `providers_title` ("Model Providers Manager", replacing
`models_title`), `providers_endpoint`, `providers_endpoint_reset`,
`providers_key`, `providers_refresh_models`, `providers_models_count`,
`providers_models_error`, `providers_unconfigured`, `providers_local_no_key`.

Agents table: `agents_tbl_provider_scope`, `agents_tbl_agent`,
`agents_tbl_model`, `agents_tbl_temp`, `agents_tbl_max_tokens`,
`agents_tbl_timeout`, `agents_tbl_no_model`, `agents_tbl_model_search`,
`agents_tbl_clash`, `agents_val_temp_range`, `agents_val_tokens_range`,
`agents_val_timeout_range`.

Migration / housekeeping: `migrate_providers_done`, `migrate_key_discarded`,
`leaderboard_pruned_untested`.

Retired: the profile-centric strings in the current Models Manager and the
proficiency-button labels on the two surfaces that lose it.

### 3.5 Migration (`migrate_profiles_to_providers`)

Runs once per project open, before the first resolution, and is idempotent (it
is a no-op when no agent holds a `model_profile` and no legacy profiles are
present).

1. For each agent with `model_profile: Some(id)` resolving to a profile: copy
   `provider`, `endpoint`, `model`, `temperature`, `max_tokens`, `timeout_secs`
   onto the agent; set `model_profile = None`. A **dangling** reference leaves
   the agent with no model (today's behaviour) rather than inventing one.
   `no_model` agents are skipped entirely (R28).
2. For each distinct provider among the profiles, create a `ProviderConfig`.
   Endpoint: the profile with `endpoint_user_edited == true` wins; if several
   disagree, the most recently keyed one wins and the rest are reported; if none,
   the `PROVIDERS` default.
3. Credentials: among that provider's profile slots, choose the one with the
   greatest `api_key_saved_at` (R25). Re-key its timestamp, native marker and any
   in-session secret to `providerkey::<id>`. Every other slot for that provider
   is named in the Output panel together with the provider (R26).
4. Return a `MigrationReport { agents_migrated, providers_created, discarded: Vec<(provider, profile_label)>, endpoint_conflicts }` for the Output panel. No modal, no prompt (R27).

Leaderboard entries are not touched by migration at all — they are already keyed
`(provider, model)` — which satisfies R29 by construction.

## 4. Key decisions & alternatives

**D1 — Provider configuration is machine-wide, not per-project.**
*Why:* a credential and the host it authenticates against belong together, and
credentials are already machine-local. Configure Anthropic once and every project
uses it, which is the whole point of R1. It also narrows the machine-vs-project
split that 1.61.6 had to document as a caveat.
*Rejected:* keeping it per-project in `cobolt.toml`, as profiles are today. It
preserves the current shape and lets a project carry its endpoints — but the
project cannot carry the key, so the developer must visit the manager on every
new machine anyway, and they would then have to do it once per project instead of
once. **Flagged for operator confirmation (Q6): this is the one decision here
that changes project portability.**

**D2 — `ModelProfile` is kept deserialisable for one release, never written.**
*Why:* migration must be repeatable — a project opened by an older build, or
restored from the `llm_config.json` backup path, can still present profiles.
*Rejected:* deleting the type outright, which makes an old `cobolt.toml`
unreadable rather than merely legacy. (Resolves spec Q2 as recommended.)

**D3 — A successful model-list fetch is the credential validation.**
*Why:* the fetch already exercises the key against the provider; a second probe
would double the requests to prove the same thing.
*Rejected:* an explicit "Test connection" probe on save. The existing test-request
control stays available for diagnosis, but it is not on the configuration path.
(Resolves spec Q3 as recommended.)

**D4 — A reachable Ollama endpoint counts as configured (R6a).**
*Why:* a local runtime has no credential to enter, and requiring an "enable"
click would be a step whose only function is to say "yes, still local".
*Rejected:* an explicit enable toggle. (Resolves spec Q4 as recommended.)

**D5 — "Apply to every specialist" is blocked, not partially applied, when the
model is Grace's or the Judge's.**
*Why:* a partial apply leaves the developer guessing which specialists took the
change; the clash message names the reserving role and is actionable.
*Rejected:* applying to non-clashing specialists only. (Resolves spec Q5 as
recommended.)

**D6 — The model combobox is searchable, not a plain list.**
*Why:* OpenRouter lists 300+ models; a flat dropdown is unusable at that length.
*Rejected:* truncating the list, which hides exactly the model the developer
came for.

**D7 — Separation is validated at pick time and re-reported on open.**
*Why:* R23 wants the clash named when the selection is made, but a project can
also arrive already clashing (edited outside the IDE, or migrated from profiles
that shared a model). Validating only at pick time would let that state sit
silently.
*Rejected:* pick-time only.

## 5. Risks & mitigations

- **Blast radius.** `model_profiles` has 99 references across six files, and
  `llm.rs` is 8.1k lines. → Stage the work: data layer + migration with tests
  first, then one UI surface at a time, keeping the build green at each step.
  `/tasks` should order it that way.
- **Silent credential loss on migration.** Choosing one key per provider is
  lossy by construction. → R26's Output report names every discarded slot;
  because keys do not persist today, the practical loss is a timestamp and a
  vault marker, not a working credential. Call this out in the guide.
- **Revising a fix that shipped two commits ago.** 1.61.6 added
  `prune_unregistered`; this replaces it with `prune_untested_orphans`. → The
  new rule is strictly safer (it can no longer delete a row with scores). The
  changelog entry must say so plainly, and the f=96 post should note the
  behaviour supersedes 1.61.6's.
- **A provider that answers `/models` with hundreds of entries** makes the agents
  table's dropdown slow to build every frame. → Cache the fetched list per
  provider on `LlmConfig`; filter on a search string; never re-fetch during
  layout.
- **An agent left mid-migration** if the app is killed between agent writes.
  → Migration writes agents first and only then drops the profiles; a re-run
  finishes the job because step 1 is keyed on `model_profile.is_some()`.
- **Top-level `llm.provider`/`llm.model` orphaned.** Direct AI surfaces use the
  top-level default, seeded today by `ensure_default_model_from_profiles`.
  → Replace with `ensure_default_model_from_agents` (Grace first, then any agent
  with a model, then any configured provider's first model).
- **Six-language churn.** ~24 new keys plus retirements. → Add every key in one
  pass with the `i18n_tests` sweep green before UI work starts.

## 6. Test strategy

All in `cobolt-ide` (`cargo test -p cobolt-ide`), following the existing
`agents_db::tests` / `leaderboard::tests` / `i18n_tests` patterns.

**Migration (`agents_db::tests`)** — the highest-value tests, written first:
- `profiles_migrate_onto_their_agents` — three agents on two profiles end with
  the same provider/model/temp/tokens/timeout they resolved to before, and
  `model_profile == None`. Reports the before/after table.
- `the_newest_key_per_provider_survives` — two profiles on one provider with
  `api_key_saved_at` 100 and 200 → `providerkey::<id>` carries the 200 slot's
  timestamp; the discarded profile is named in the report.
- `a_dangling_profile_reference_leaves_the_agent_unconfigured`.
- `an_explicit_no_model_choice_survives_migration` (R28).
- `migration_is_idempotent` — running twice changes nothing the second time.
- `endpoint_conflicts_are_reported_not_guessed`.

**Resolution (`agents_db::tests`)**
- `an_agent_resolves_from_its_own_fields_and_its_providers_endpoint`.
- `two_agents_on_different_providers_resolve_independently` (R11, AC9).

**Separation (`agents_db::tests`, extending the existing cases)**
- `a_specialist_on_graces_model_still_clashes_after_the_redesign` (R21).
- `the_judge_may_share_graces_model_until_a_specialist_arrives` (R22) — the
  existing case, re-pointed at the new resolution path.

**Leaderboard (`leaderboard::tests`)**
- `a_row_with_runs_is_never_pruned` (R19) — the guard against the 1.61.6 rule
  being reintroduced.
- `an_untested_row_no_agent_uses_is_pruned_and_named` (R18).
- `assigned_models_populate_the_board` (R17).

**i18n (`i18n_tests`)** — the existing completeness sweep covers the new keys;
add `no_surface_still_says_models_manager` asserting the old label is gone from
all six tables (AC1).

**UI behaviour (`agents_modal` / `models_modal` tests, existing render harness)**
- `the_provider_scope_does_not_change_a_stored_agent_provider` (R11, AC8).
- `out_of_range_tuning_values_are_rejected_and_the_prior_value_stands` (R14).
- `no_proficiency_control_exists_outside_the_leaderboard` (R16, AC13) — asserts
  the action fields are gone from both action structs.

**Manual / visual** (the operator drives the IDE; I do not):
1. Launch, open the Model Providers Manager, confirm the title in all six
   languages and that no surface says "Models Manager".
2. Enter one provider key → models populate; assign different models to Grace
   and a specialist; confirm both run.
3. Point a specialist at Grace's model → clash badge with the reserving role.
4. Open the Leaderboard: assigned models listed, a scored row survives
   reassignment, an unscored orphan disappears and is named in Output.
5. Open a pre-048 project: agents unchanged, no prompt, migration lines in
   Output.

**Reporting.** Each new test prints a compact summary (what was migrated, which
slot won, which rows were pruned) rather than a bare pass/fail, per the steering
rule on quantified test output.

## 7. Steering compliance

- [ ] **i18n:** ~24 new `Tr` fields in all six languages; retired profile strings
      removed, not orphaned. `i18n_tests` green.
- [x] **Generated-code banner + regenerate-on-action:** not touched — this change
      reaches no control, property, method, event or codegen path.
- [ ] **System KB:** no `cobolt-compiler` doc table is expected to change. If the
      implementation touches one, the same change runs
      `cargo run -p cobolt-ide --example build_chunked_kb` and commits
      `assets/knowledge/chunked.data`.
- [ ] **English dev guide updated:** Models Manager → Model Providers Manager,
      the agents table, Leaderboard-only proficiency testing, and a migration
      note. `-es/-pt/-jp/-cn` untouched.
- [ ] **Fix vs feature:** **feature** — its own commit on the `features` branch,
      announced on f=96 with the `[Noticia]` prefix, never mixed with a fix.
- [ ] **Version: `1.61.8`** — settled by the operator 2026-08-09. Only the
      operator raises `x`/`y`; an agent bumps `z`, feature or not.
      `specs/steering/tech.md` still says a feature bumps the minor and is
      **corrected in the same change** so the two documents stop contradicting
      each other.
- [x] **No "cobolt" in user-facing text; COBOL identifiers English:** the change
      is IDE-configuration only and adds no COBOL-facing text.

## 8. Questions resolved before `/tasks`

Both settled by the operator on 2026-08-09; no open questions remain.

- **Q1 — version.** Ships as **`1.61.8`**. Only the operator raises `x`/`y`;
  agents bump `z`. `specs/steering/tech.md` is corrected in the same change.
- **Q6 — provider configuration store.** **Machine-wide, in `llm_config.json`**,
  beside the credentials (decision D1 as recommended). Configure a provider once
  and every project uses it. `cobolt.toml` keeps `[ai].model_profiles` only as
  read-only legacy input for the migration and stops being written.
