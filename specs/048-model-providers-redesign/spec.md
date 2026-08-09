<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — Model Providers redesign

- **Status:** draft → approved
- **Folder:** specs/048-model-providers-redesign/
- **Author:** Anthropic Claude Codex Agent (for Emerson Lopes)   **Date:** 2026-08-09

## 1. Overview

Configuring a model in PowerRustCOBOL AI today means creating a **model
profile**: a named record holding a provider, an endpoint, one model id, an API
key, and three tuning values (temperature, max output tokens, timeout). An
agent then *references* a profile. Adding a second model from a provider you
have already paid for means building a whole second profile and pasting the
same key again; comparing two models means keeping two near-identical records
in step.

This spec replaces that unit of configuration. The developer configures a
**provider** once — its API key and its endpoint — and from that moment every
model the provider offers is selectable. Choosing which model an agent runs on,
and how hot, how long and how large its answers may be, becomes a property of
**the agent**, edited where the agents live: one row per agent in the Agents
Manager, in a table the developer can read top to bottom. Proficiency testing
stops being a button on a model record and belongs solely to the Leaderboard,
which already ranks by `(provider, model)`.

The change is smaller than it looks. `AgentDef` still carries
`provider`/`endpoint`/`model`/`temperature`/`max_tokens`/`timeout_secs` — spec
031 left them in place but dormant behind `model_profile`. This spec wakes them
up and retires the profile layer above them.

## 2. Goals / Non-goals

**Goals**

- One configuration record per **provider** (API key + endpoint), not per model.
- Every model a configured provider offers is selectable without further setup.
- Per-agent model, temperature, output-token cap and timeout, edited in one
  table in the Agents Manager.
- Proficiency testing lives in the Leaderboard and nowhere else.
- Existing projects migrate silently and losslessly, apart from duplicate API
  keys, which are resolved by a stated rule and reported.
- The spec 040 R10 model-separation rule keeps holding, unchanged in meaning.

**Non-goals**

- No change to the set of supported providers (`llm::PROVIDERS`), to the wire
  protocols, or to `spawn_list_models`' discovery logic.
- No change to how the proficiency test itself scores a model, or to the
  Leaderboard's four boards and tiering.
- No change to where secrets live (machine-local store, never `cobolt.toml`).
- Not a redesign of agent prompts, companions, skills or routing.
- No new provider-side features (no cost tracking, no rate-limit UI).

## 3. User stories

- As a developer with one Anthropic key, I want every Claude model available
  the moment the key validates, so that trying a different model is a dropdown
  choice rather than a new configuration record.
- As a developer tuning Grace, I want her model, temperature, output-token cap
  and timeout on one row, so that I can see and change her whole runtime setup
  without opening a second manager.
- As a developer comparing agents, I want one table showing every agent and the
  model it runs on, so that I can spot at a glance that two specialists are
  sharing a model or that the Judge is still unset.
- As a developer running a mixed setup, I want Grace on a cloud provider while
  specialists run local Ollama, so that I control cost without giving up
  orchestration quality.
- As a developer benchmarking, I want one place — the Leaderboard — that runs
  proficiency tests and ranks results, so that I am not testing the same model
  from three different screens.
- As an existing user, I want my current agents to keep working after the
  upgrade without reconfiguring them, so that the change costs me nothing.

## 4. Requirements (EARS)

### Provider configuration

- **R1 (ubiquitous):** The system shall store configuration per provider as a
  record of `{ provider id, endpoint, API key }`, and shall not require a
  per-model configuration record.
- **R2 (ubiquitous):** The system shall present this surface under the name
  **"Model Providers Manager"** in every language, replacing the name "Models
  Manager".
- **R3 (ubiquitous):** The system shall default a provider's endpoint to
  `PROVIDERS[].default_endpoint` and shall allow the developer to override it
  per provider.
- **R4 (event):** When the developer saves a provider's API key, the system
  shall fetch that provider's model list via `llm::spawn_list_models` and cache
  the result for selection.
- **R5 (event):** When a model-list fetch fails, the system shall report the
  failure on the provider's row and shall still allow a model id to be entered
  by hand.
- **R6 (state):** While a provider has no API key stored, the system shall
  treat that provider as unconfigured and shall offer none of its models for
  selection.
- **R6a (optional):** Where a provider needs no API key to answer (a local
  Ollama endpoint), the system shall treat a reachable endpoint as sufficient
  configuration and shall offer its models.
- **R7 (constraint):** The system shall not write any API key to `cobolt.toml`,
  to a `.cfrm`, or to any other file inside the project.

### Per-agent runtime settings

- **R8 (ubiquitous):** The system shall store each agent's provider, model,
  temperature, maximum output tokens and timeout on the agent itself.
- **R9 (ubiquitous):** The Agents Manager shall present a table with one row per
  agent — including Grace and the COBOL Proficiency Judge — with the columns
  **Agents · Models · Temp · Output Tokens · Timeout**.
- **R10 (ubiquitous):** The Agents Manager shall present a **model-provider
  combobox above the table** that scopes which provider's models the Models
  column offers while configuring.
- **R11 (constraint):** The provider combobox shall not change any agent's
  stored provider. Each agent shall retain the provider it was configured with,
  so agents may run on different providers simultaneously.
- **R12 (event):** When the developer picks a model for an agent, the system
  shall store both the model id and the provider currently scoped in the
  combobox, together with that provider's endpoint.
- **R13 (ubiquitous):** The system shall allow an agent to be set explicitly to
  **(no model)**, and shall keep that choice across project open (the existing
  `AgentDef::no_model` marker).
- **R14 (ubiquitous):** The system shall validate temperature, output tokens and
  timeout on entry and shall reject values outside each field's supported range.

### Proficiency testing

- **R15 (ubiquitous):** The system shall offer proficiency testing only in the
  Leaderboard.
- **R16 (constraint):** The system shall not offer a proficiency-test action in
  the Model Providers Manager or in the Agents Manager.
- **R17 (event):** When the Leaderboard opens, the system shall list a row for
  every model currently assigned to an agent and for every model that has a
  recorded test run.
- **R18 (event):** When the Leaderboard opens, the system shall remove only
  those rows that have **no recorded runs** and are assigned to no agent, and
  shall name each removal in the Output panel.
- **R19 (constraint):** The system shall not remove a Leaderboard row that has
  one or more recorded runs, whatever the current agent assignments.
- **R20 (ubiquitous):** The Leaderboard shall allow the developer to add any
  model of a configured provider in order to test it.

### Model separation (spec 040 R10, preserved)

- **R21 (constraint):** The system shall not allow an enabled specialist agent
  to run the model assigned to Grace, nor the model assigned to the COBOL
  Proficiency Judge.
- **R22 (optional):** Where no enabled specialist is on that model, the system
  shall allow the COBOL Proficiency Judge to share Grace's model.
- **R23 (event):** When a model selection would break R21, the system shall
  report the clash naming the agent, the model and the role that reserves it,
  using the existing `ModelSeparation` / `ModelClash` reporting.

### Migration

- **R24 (event):** When a project configured with model profiles is opened for
  the first time after the upgrade, the system shall convert each agent's
  referenced profile into that agent's own provider, endpoint, model,
  temperature, output-token cap and timeout, without developer action.
- **R25 (event):** When migrating API keys, the system shall keep, for each
  provider, the **most recently stored** key among that provider's profile keys.
- **R26 (event):** When migration discards a key under R25, the system shall
  name the affected provider and the discarded profile in the Output panel.
- **R27 (constraint):** Migration shall not prompt the developer at startup.
- **R28 (ubiquitous):** The system shall preserve the developer's explicit
  **(no model)** choices through migration.
- **R29 (ubiquitous):** The system shall preserve all existing Leaderboard
  entries, scores and run counts through migration.

## 5. Acceptance criteria

**Provider configuration**

- [ ] AC1 — The manager is titled "Model Providers Manager" in EN/ES/PT/JA/ZH/FR;
      no surface anywhere still reads "Models Manager". (R2)
- [ ] AC2 — Saving a valid Anthropic key makes every Anthropic model offered by
      the listing endpoint selectable for an agent, with no further setup. (R1, R4)
- [ ] AC3 — A provider's endpoint defaults to its `PROVIDERS` entry and survives
      an edit + reopen. (R3)
- [ ] AC4 — With the model listing endpoint unreachable, the row shows the error
      and a hand-typed model id is still accepted and usable. (R5)
- [ ] AC5 — A provider with no key offers no models; an Ollama endpoint that
      answers offers its models with no key. (R6, R6a)
- [ ] AC6 — After configuring every provider, grepping the project directory
      finds no API key in any file. (R7)

**Agents table**

- [ ] AC7 — The Agents Manager shows one row per agent, Grace and the Judge
      included, with the five specified columns. (R9)
- [ ] AC8 — Switching the provider combobox changes the models offered in the
      Models column and changes no agent's stored provider; agents already
      configured on another provider still show their own model. (R10, R11)
- [ ] AC9 — Grace on Anthropic and a specialist on Ollama run correctly in the
      same session. (R11, R12)
- [ ] AC10 — Setting an agent to (no model) survives closing and reopening the
      project. (R13, R28)
- [ ] AC11 — Temperature above its maximum, a zero output-token cap and a zero
      timeout are each rejected with a message, and the prior value stands. (R14)
- [ ] AC12 — Changing an agent's temperature is reflected in the next request
      that agent makes. (R8)

**Proficiency testing**

- [ ] AC13 — No proficiency-test control exists in the Model Providers Manager
      or the Agents Manager. (R16)
- [ ] AC14 — A proficiency run started from the Leaderboard records its scores
      against the `(provider, model)` row. (R15)
- [ ] AC15 — Opening the Leaderboard lists every model assigned to an agent and
      every model with recorded runs. (R17)
- [ ] AC16 — A row with runs is retained after its agent is reassigned; a row
      with zero runs and no agent is removed and named in the Output panel.
      (R18, R19)

**Separation**

- [ ] AC17 — Assigning Grace's model to an enabled specialist raises a clash
      naming the agent, the model and `Grace`. (R21, R23)
- [ ] AC18 — The Judge on Grace's model with no specialist there raises no
      clash; adding a specialist to that model raises one. (R21, R22)

**Migration**

- [ ] AC19 — A project built on profiles opens with every agent on the same
      provider, model, temperature, output-token cap and timeout it had before,
      with no prompt shown. (R24, R27)
- [ ] AC20 — Two profiles on one provider holding different keys migrate to the
      most recently stored key, and the Output panel names the provider and the
      discarded profile. (R25, R26)
- [ ] AC21 — Leaderboard entries, scores and run counts are identical before and
      after migration. (R29)

## 6. Constraints & steering check

**i18n (6 languages).** Substantial. The rename (R2) touches `models_title` and
every string that names the manager. The agents table adds column headers
(Agents / Models / Temp / Output Tokens / Timeout), the provider-scope combobox
label, validation messages (R14), migration reports (R26) and the leaderboard
housekeeping line (R18). Every new `Tr` field must be filled in EN/ES/PT/JA/ZH/FR;
no hard-coded literals. Retired profile strings must be removed, not left
dangling.

**Generated-code / regenerate contract.** No impact. This spec touches no
control, property, method, event, or code generation path; no `.cbl` output
changes and `regenerate_all_forms` is untouched.

**System KB.** No impact expected: the KB publishes the compiler's control /
property / method / event documentation, and this change alters none of it. If
implementation nevertheless edits any `cobolt-compiler` doc table, the same
change must run `cargo run -p cobolt-ide --example build_chunked_kb` and commit
the regenerated `assets/knowledge/chunked.data`.

**Docs (English guide).** Required. `docs/developers-guide-en.md` describes the
Models Manager, per-model configuration and where proficiency testing is run.
Those sections must be rewritten for provider configuration, the agents table
and the Leaderboard-only test, including the migration note. Translations are
user-maintained and must not be edited.

**Fix vs feature.** **Feature** — this is new IDE capability beyond the existing
scope, so it goes in its own commit on the `features` branch and is announced on
forum f=96 (prefix `[Noticia]`), never mixed with a fix commit.

**Versioning.** ⚠️ Conflict to settle before `/plan` completes: `tech.md` says a
feature bumps the **minor** (`y`), while the operator's standing rule is that
**only the operator raises `x` or `y`** — agents bump the fix number. Recorded
as Q1 below; the default taken is the operator rule (bump `z`).

**Other steering.** No user-facing "cobolt" text; no COBOL identifier or source
language change; user code untouched. `AgentDef` fields are revived, not added,
so on-disk agent manifests stay backward-readable.

## 7. Open questions

- **Q1 — Version number.** `tech.md` says features bump the minor; the
  operator's standing rule says agents never do. Default assumed: bump `z`
  (a fix-numbered feature release). Operator to confirm, and to correct
  `tech.md` if the standing rule wins.
- **Q2 — `ModelProfile` removal vs deprecation.** Should the type and its
  `serde` shape be deleted outright once migration has run, or retained as a
  read-only legacy structure so a project can be opened by an older IDE build?
  Recommendation: keep the deserialisation path for one release so migration is
  repeatable, and stop writing profiles.
- **Q3 — Provider validation.** Should saving a key actively validate it (a
  cheap probe) before the models are offered, or is a successful model-list
  fetch (R4) validation enough? Recommendation: the fetch is enough — it already
  exercises the credential.
- **Q4 — Ollama and the provider list.** Local Ollama has no key. Should it
  appear as always-configured when its endpoint answers, or still require an
  explicit "enable" action? R6a assumes always-configured-when-reachable.
- **Q5 — Bulk assignment.** The Leaderboard's existing "apply to Grace / judge /
  every specialist" actions write to agents. Applying to *every specialist*
  would immediately violate R21 if that model is Grace's or the Judge's. Should
  the action be blocked in that case, or applied to the specialists that would
  not clash? Recommendation: block with the clash message.
