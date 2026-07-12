<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — rig Agent Mesh (multi-agent AI architecture)

- **Status:** draft → approved
- **Folder:** specs/026-rig-agent-mesh/
- **Author:** PowerRustCOBOL / agent-mesh   **Date:** 2026-07-12

## 1. Overview

The dev-time AI assistant (spec 025) is a **single prompt-curated agent**: one
system prompt + a handful of skill files, one chat-completions request, one
reply parsed into a change-set. Correct behaviour depends almost entirely on
prompt curation, which does not scale — every new capability grows the prompt,
every model swap re-tunes it, and nothing *verifies* that produced COBOL is
actually something PowerRustCOBOL can compile and run. This feature replaces
the assistant's **internals** with a **multi-agent mesh built on
[rig](https://github.com/0xPlaygrounds/rig)** (rig-core, MIT, tokio-based):
an orchestrator routes developer requests to **specialist agents** (one per
PowerRustCOBOL domain — COBOL divisions/sections, data structures, file types,
statement families, run-unit behaviour, RustCOBOL extensions, forms/RAD,
project/build), and **pedantic verifier agents** — deterministic rig *tools*
wrapping the IDE's own lexer/parser/semantic/codegen crates — gate every piece
of produced code against what PowerRustCOBOL genuinely supports before the
developer ever sees it. The developer-facing surface (prompt bar, chat bubbles,
preview-then-approve, `agentic_ai/` folder) is **unchanged**; capability,
context, memory, and observability improve underneath.

## 2. Goals / Non-goals

**Goals**
- Adopt **rig** as the agent runtime: providers, agents, tools, streaming,
  embeddings — replacing the hand-rolled request path in `cobolt-ide::llm` for
  agentic features (a new IDE-independent crate, `cobolt-agents`).
- An **orchestrator** agent as the single entry point behind the existing UI,
  routing to **specialist agents**, each with a narrow system prompt, its own
  skills, its own tool set, and (optionally) its own model.
- **Specialist taxonomy** covering: COBOL divisions & sections; data structures
  (levels, PIC, REDEFINES, OCCURS, 88s); file types & ISAM engines; statement
  (verb) families; run-unit behaviour (CALL, nested programs, EXTERNAL,
  lifecycle); RustCOBOL extensions (`::` properties, member chains, control
  events, forms CALLs, charts, data binding, menus, user controls); forms/RAD
  change-sets (spec 025 vocabulary); project/build (cobolt.toml, rcrun).
- **Pedantic agents as tools**: deterministic verifiers deeply integrated with
  the IDE — syntax check (lexer+parser), semantic check, control/property/event
  schema check, data-binding guardian, capability check (supported-syntax
  matrix), headless dry-run — invocable by *any* agent, local or cloud model,
  and used as a **mandatory gate** on produced code with a bounded
  generate→verify→repair loop.
- **Declarable agents**: per-project agent definitions under `agentic_ai/`
  (resolving spec 025 Q6) — prompts, skills, model override, tool allow-list,
  including developer-declared additional pedantic agents.
- **Better context**: layered assembly (system prompt → skills → live project
  snapshot → retrieved knowledge) with a **local vector index** over the
  developer guide, skills, project sources, and past Q&A (rig embeddings; must
  degrade gracefully offline).
- **Better memory**: keep spec 025 conversation memory (R15/R16) unchanged;
  add per-project **distilled long-term memory** (facts the mesh learned about
  this project) injected via retrieval, never raw transcripts.
- **Observability**: every agent/tool invocation traced — agent name, model,
  tokens in/out, duration, verdict — streamed live into the existing AI log
  pane and persisted as JSONL per session; optional OpenTelemetry (GenAI
  semantic conventions, native in rig) export.
- **Provider parity**: everything the Settings → AI assistant page supports
  today (Ollama/local, OpenAI-compatible, OpenRouter, HuggingFace, cloud keys)
  must work through rig's provider layer with the *same* stored `LlmConfig`.

**Non-goals**
- **No UI redesign**: the prompt bar, chat bubbles, preview-then-approve flow,
  Agentic AI tree node, and settings page keep their current shape. New UI is
  limited to observability affordances (trace lines in the existing AI log).
- **Not** the runtime `AgentObject` control (generated apps) — dev-time only.
- **Not** autonomous: R8 of spec 025 stands — agents act only on a developer
  request; the mesh's internal fan-out happens strictly *within* one request.
- **Not** a cloud requirement: a single local Ollama model must still drive the
  whole mesh (albeit slower); retrieval is lexical-only (R13), so no embedding
  model or vector database exists in v1.
- **Not** replacing `agentic_ai/` customisation — it is extended, and existing
  `system-prompt.md` + `skills/` keep working (fed to the orchestrator).
- **Not** backward compatible beyond the UI: no released projects exist, so no
  legacy request path, no fallback setting, and no storage-format migration
  code is kept (R19). The UI contract (R18) is the only preserved surface.

## 3. User stories

- As a COBOL developer, I ask "read CUSTOMERS.DAT indexed by CUST-ID and show
  the rows in the grid" and the orchestrator composes the file-types agent
  (SELECT/FD/engine), the data-structures agent (record layout) and the RAD
  agent (grid binding), returning **one** coherent, previewable change-set.
- As a developer using a small local model, I still get correct code because
  every snippet was pushed through the pedantic gate and repaired against real
  parser/semantic errors before I saw it — not because the model "knew" COBOL.
- As a developer, I ask "why can't I OPEN EXTEND this relative file?" and the
  run-unit/file-types specialists answer from the capability matrix — including
  *"PowerRustCOBOL does not support X"* when that is the truth.
- As a power user, I declare a project pedantic agent ("house style: every
  paragraph name starts with the form name") in `agentic_ai/agents/`, and every
  generated handler is checked against it like the built-in gates.
- As a maintainer, I open the AI log and see exactly which agents ran, with
  which models, token counts, how many repair iterations the gate needed, and
  which check failed — instead of one opaque request/response pair.

## 4. Requirements (EARS)

**Architecture & runtime**
- **R1 (ubiquitous):** The system shall provide the agent mesh in a new crate
  `cobolt-agents` built on rig-core, independent of `cobolt-ide` (no egui
  dependency), so the mesh is reusable by other hosts (rcrun, future mobile).
- **R2 (ubiquitous):** The crate shall run rig's tokio runtime on dedicated
  background thread(s) and expose the same channel-based, cancellable,
  non-blocking interface the IDE uses today (`spawn_*` → `Receiver`), keeping
  the egui thread free (spec 025 R12 preserved).
- **R3 (ubiquitous):** The mesh shall consume the existing `LlmConfig`
  (provider id, endpoint, key, model) unchanged; every provider currently
  offered in Settings shall work through rig (Ollama/local and OpenAI-
  compatible endpoints included). No new mandatory connection settings.

**Orchestration & specialists**
- **R4 (event):** When the developer submits a request, an **orchestrator
  agent** shall classify and route it; for simple requests it shall answer
  directly **without** fan-out (cost/latency guard), and for composite requests
  it shall delegate to one or more specialists and merge their results into a
  single reply/change-set.
- **R5 (ubiquitous):** The system shall ship built-in specialist agents, one
  per domain, at minimum: `cobol-identification`, `cobol-environment`,
  `cobol-data` (WORKING-STORAGE / FILE SECTION / LINKAGE), `cobol-procedure`,
  `data-structures`, `file-types` (incl. ISAM engines rust / rm-cobol85 /
  fujitsu / redb), **one statement agent per supported verb** (see R5a),
  `run-unit`, `rustcobol-extensions`, `forms-rad` (spec 025 change-set
  vocabulary), `project-build`. Each has its own preamble, skill set, tool
  allow-list, and optional model override.
- **R5a (ubiquitous):** Statement agents shall be **per-verb** (one agent per
  supported COBOL verb / RustCOBOL statement), **materialized from the verb
  support matrix** at build time — a shared statement-agent template plus a
  per-verb knowledge card (syntax forms, supported/unsupported clauses,
  diagnostics, examples from the verb test matrix) — not hand-written prompts.
  Consolidating verbs into family agents is permitted **only** by a follow-up
  change carrying measured evidence (hallucination/error rate on the statement
  corpus with the pedantic gate active, family ≤ per-verb).
- **R6 (ubiquitous):** Specialists shall be exposed to the orchestrator as
  **rig tools** (agents-as-tools), so composition, parallel fan-out (bounded),
  and result merging use one uniform mechanism.
- **R6a (ubiquitous):** Concurrent specialist calls shall be capped by provider
  class — default **2 for local endpoints** (a local server timeshares one
  model; more concurrency thrashes it) and **4 for cloud providers** (rate
  limits/cost) — both adjustable in `agentic_ai/` config. The **chat receives
  one merged final reply** per request (preview-then-approve needs a single
  coherent change-set); live per-agent progress streams into the AI activity
  log, not the chat.

**Pedantic gate**
- **R7 (ubiquitous):** The system shall provide **deterministic pedantic
  tools** wrapping the compiler crates in-process — at minimum:
  `check_syntax` (tokenize+parse, exact diagnostics), `check_semantics`
  (analyze), `check_control_schema` (control types / property keys / event
  names), `check_data_binding` (binding guardian), `check_capability`
  (supported-syntax matrix + RustCOBOL extension list), `dry_run` (R7a).
- **R7a (ubiquitous):** `dry_run` shall execute candidate COBOL in a **spawned
  `rcrun` child process** (never in the IDE process): working directory set to
  a per-invocation temp sandbox, hard wall-clock timeout (default 5 s, then
  kill), and a new `rcrun` sandbox mode that disables network/database/system
  built-in CALLs and confines file I/O to the sandbox directory. Rationale:
  model-generated code is untrusted input; a runaway loop, OOM, or hostile
  CALL must never take down or touch the IDE — the same process-isolation
  principle already adopted for Run Form / Debug Form.
- **R8 (event):** When any agent (built-in or declared, local or cloud model)
  produces COBOL source or a change-set, the mesh shall pass it through the
  applicable pedantic tools **before** it reaches the preview; on failure the
  producing agent shall receive the verbatim diagnostics and retry, up to a
  configurable bound (default 3); an unrepaired result shall be surfaced as an
  error with its diagnostics, and nothing shall be applied.
- **R9 (constraint):** Pedantic tools shall be pure/deterministic (no LLM, no
  network); their verdicts shall be reproducible from the same input.

**Declarable agents (resolves 025-Q6)**
- **R10 (ubiquitous):** Agent definitions shall live in the project's
  `agentic_ai/agents/` folder — **one markdown file per agent with YAML
  front-matter** (the format used by mainstream agent tooling, so definitions
  are portable and familiar): front-matter declares `name`,
  `role: specialist | pedantic`, `skills:` (references into
  `agentic_ai/skills/`), `tools:` (allow-list), optional `model:` override;
  the markdown **body is the preamble**. For pedantic agents the body is the
  rule text evaluated by an LLM, and front-matter may bind an optional
  deterministic tool. **Both roles — declared specialists and declared
  pedantic agents — are in scope for v1.** Built-in defaults are seeded like
  R19 of spec 025 (missing files re-seeded, existing never overwritten).
- **R11 (state):** While a project declares agents, the Agentic AI tree node
  shall list them (same open/edit/save behaviour as other `agentic_ai/`
  files), and the mesh shall hot-reload definitions when files change on save.

**Context & memory**
- **R12 (ubiquitous):** Request context shall be assembled in layers: effective
  system prompt (existing), agent skills, a **live project snapshot** (current
  form inventory / selection / open file / diagnostics, as today), and
  **retrieved knowledge** from a local index over: `docs/developers-guide-en`,
  `agentic_ai/skills/`, project sources, and past Q&A logs.
- **R13 (ubiquitous):** Retrieval shall be **lexical-only** in this feature:
  a local BM25-style inverted index (tokenized, stemmed, TF-IDF/BM25 ranked)
  plus a **curated COBOL/RustCOBOL synonym table** (e.g. "keyed file" →
  INDEXED, "grid" → DataGrid) to close the paraphrase gap. No embedding
  model, no vector store, no network: the index builds and queries fully
  offline and adds no model downloads. Embedding-based retrieval is a
  possible follow-up spec, not part of v1.
- **R14 (ubiquitous):** The mesh shall provide conversation memory with the
  same **UI behaviour** as spec 025 (per-form/project history, persists across
  restarts, excludes prompt/skills/snapshot) — the storage format is free to
  change (no backward compatibility required). Additionally the mesh shall
  maintain per-project **distilled memory** (`agentic_ai/memory/`): short
  factual notes the orchestrator elects to save (e.g. "this project targets
  the redb ISAM engine"), injected via retrieval; raw transcripts shall never
  be injected wholesale.

**Observability**
- **R15 (ubiquitous):** Every agent invocation and tool call shall emit a trace
  event — timestamp, agent, model, prompt/response token counts, duration,
  outcome (incl. pedantic verdict and repair-iteration count) — streamed into
  the existing AI activity log pane, and persisted as JSONL under the project's
  `data/` (rotating, size-capped).
- **R16 (optional):** Where enabled in settings, traces shall export via
  OpenTelemetry using rig's GenAI semantic-convention support; off by default.
- **R17 (constraint):** Trace payloads shall never include the developer's API
  keys; full prompts/responses are persisted only when the existing verbose AI
  log setting is on.

**Compatibility & rollout**
- **R18 (ubiquitous):** The developer-facing **UI behaviour** contract of spec
  025 (prompt bar, chat bubbles, preview-then-approve, single undoable
  change-set, R8 non-autonomy, R9 validation, R10 generated-code contract,
  R11 i18n) shall hold unchanged under the mesh — the UI is the **only**
  compatibility surface.
- **R19 (ubiquitous):** The mesh **replaces** the legacy single-agent request
  path outright: `cobolt-ide::llm`'s agent request plumbing is deleted in the
  same change, with no fallback setting, no dual code paths, and no data-format
  migration shims (there are no released projects to migrate; internal file
  formats — history, logs, indexes — may change freely).
- **R20 (constraint):** A single configured local model shall be sufficient to
  operate the entire mesh (orchestrator + specialists sharing it); per-agent
  model overrides are optional, never required.
- **R21 (constraint):** New user-facing strings (trace labels, gate errors)
  shall be `Tr` fields translated in all six languages.

## 5. Acceptance criteria

- [ ] **AC1** — With only Ollama + one local model configured, a composite
      request ("indexed file + record + grid binding") produces one merged,
      previewable change-set; the AI log shows orchestrator + ≥2 specialists +
      pedantic gate entries. (R3, R4, R5, R6, R15, R20)
- [ ] **AC2** — A trivial request ("what does PERFORM VARYING do?") is answered
      by the orchestrator with **zero** specialist fan-out, verified in the
      trace. (R4, R15)
- [ ] **AC3** — Feeding the mesh a model that emits invalid COBOL (forced via a
      canned mock provider in tests) shows repair iterations in the trace and
      either a gate-passing result or a blocked error with verbatim parser
      diagnostics; nothing invalid ever reaches the preview. (R7, R8, R9)
- [ ] **AC4** — `check_syntax`/`check_semantics` verdicts on a corpus of
      valid/invalid samples are identical across two runs and match
      `rcrun check`. (R9)
- [ ] **AC5** — Declaring a pedantic agent in `agentic_ai/agents/` makes its
      check run on the next generated handler (trace proves it); deleting the
      file removes it after save (hot reload). (R10, R11)
- [ ] **AC6** — Fully offline (no network, no embedding anything), a query
      phrased colloquially ("how do I read a keyed file into the grid?")
      retrieves the INDEXED + DataGrid passages via the lexical index +
      synonym table. (R13)
- [ ] **AC7** — A fact saved to distilled memory in one session influences a
      later session's answer (trace shows the retrieved note). (R14)
- [ ] **AC8** — JSONL trace files appear per session, rotate at the size cap,
      and contain no API keys; with verbose off, no full prompt bodies. (R15,
      R17)
- [ ] **AC9** — The legacy single-agent request path is gone: no fallback
      setting exists, and code search finds no dual request plumbing in
      `cobolt-ide::llm` beyond provider/config/UI helpers. (R19)
- [ ] **AC10** — All spec 025 acceptance criteria that describe **UI
      behaviour** still pass with the mesh (regression suite). (R18)
- [ ] **AC14** — A statement agent exists for every verb in the support
      matrix, generated from the shared template + per-verb knowledge card
      (spot-check: MOVE, PERFORM, STRING, READ, COMPUTE); asking a per-verb
      agent about an unsupported clause answers "not supported" with the
      matrix citation. (R5a)
- [ ] **AC15** — A declared **specialist** (not just pedantic) agent in
      `agentic_ai/agents/` participates in routing after file save. (R10,
      R11)
- [ ] **AC11** — Every provider selectable in Settings completes a mesh request
      (live for Ollama/OpenAI-compatible; recorded/mocked for keyed clouds in
      CI). (R3)
- [ ] **AC12** — New strings exist in all six i18n tables. (R21)
- [ ] **AC13** — The egui thread never blocks during a mesh request (existing
      responsiveness test pattern); cancel works mid-fan-out. (R2)

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — settings toggle, gate error surface, trace
  labels. All six languages (tech.md hard constraint).
- **Generated-code / regenerate contract:** Unchanged by design (R18); the
  pedantic gate *strengthens* it — produced code must parse/analyze with the
  real crates before preview.
- **Docs (English guide):** Yes — the "AI development agent" section gains
  subsections: how the mesh works, declaring agents, pedantic gates, reading
  the AI trace. Translations are user-maintained (do not edit).
- **Fix vs feature:** **Feature** per tech.md (minor bump + CHANGELOG); the
  operator's pre-prod convention (everything ships as a fix / z-bump, forum
  f=97) applies at release time — confirm at commit.
- **New dependencies:** `rig-core` (MIT — compatible with Apache-2.0), `tokio`
  (already transitively present via tooling? verify; MIT), optional local
  embedding backend, optional OTel exporter crates (behind a feature flag).
  Binary-size and compile-time impact must be measured in `/plan`; rig's
  provider features shall be gated so unused providers aren't compiled in.
- **Offline-first:** Hard constraint carried from today: Ollama-only setups
  must remain fully functional (R13, R20).

## 7. Open questions

Resolved during clarification (2026-07-12, operator):
- ~~Q2 (statement granularity)~~ → **per-verb agents** (R5a); family
  consolidation only with measured evidence that the pedantic gate keeps the
  hallucination rate at per-verb levels.
- ~~Q3 (embeddings)~~ → **lexical-only** retrieval in v1 (R13): BM25-style
  local index + curated COBOL synonym table; embeddings deferred to a possible
  follow-up spec.
- ~~Q5 (agent format / scope)~~ → **one markdown file per agent with YAML
  front-matter** (R10); declared specialists **and** pedantic agents both in
  v1.
- ~~(compatibility)~~ → **no backward compatibility** beyond the UI (R19):
  legacy request path deleted, no fallback setting, storage formats free to
  change.

Resolved during second clarification (2026-07-12, operator):
- ~~Q1 (rig pinning)~~ → **track the latest stable rig-core release**: pin the
  exact latest stable at implementation time (`=x.y.z`), upgrade deliberately
  when a new stable lands, and keep all rig types behind the `cobolt-agents`
  adapter layer so upgrades never leak breakage into the IDE.
- ~~Q4 (dry_run sandbox)~~ → **spawned `rcrun` child process** with temp-CWD
  sandbox, 5 s kill timeout, and a sandbox mode disabling network/DB/system
  CALLs (R7a). Chosen over in-process execution because model-generated code
  is untrusted input and process isolation is the project's established
  pattern (Run Form / Debug Form).
- ~~Q6 (token budgets)~~ → **not in v1.** No explicit budget system; the
  per-verb knowledge cards (R5a) are inherently small, and providers apply
  their own limits. Revisit in a follow-up spec if traces show context
  overflows on small local models.
- ~~Q7 (parallelism)~~ → caps **2 local / 4 cloud**, configurable; **one
  merged reply** to the chat, per-agent progress in the AI log (R6a).
- ~~Q8 (capability matrix source)~~ → **follow the Anthropic convention:
  knowledge as markdown files with YAML front-matter** (the same shape as
  skills/agents). One file per verb under a repo `knowledge/verbs/` corpus:
  front-matter carries the machine-readable support flags (clauses
  supported/unsupported, since-version), the body carries syntax, examples,
  and pitfalls. A build step parses front-matter into the `check_capability`
  matrix, and the same file **is** the R5a knowledge card — one human-editable
  source feeding both, with a CI check that every verb in the test matrix has
  a card and vice versa.

No open questions remain — the spec is ready for approval and `/plan`.
