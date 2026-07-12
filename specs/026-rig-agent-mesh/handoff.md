<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Handoff — continuing the Agentic AI revamp (spec 026)

For the next agent picking up this work. State as of 2026-07-12.

## Where things stand

- `specs/026-rig-agent-mesh/spec.md` is **complete: zero open questions**.
  Every clarification was answered by the operator and folded into the
  requirements (see §7 for the resolution log). The spec is **not yet
  formally approved** — confirm approval with the operator before `/plan`.
- Nothing has been implemented. No rig dependency exists yet anywhere in the
  workspace. The spec folder is uncommitted at handoff time (check `git
  status`).

## The workflow you must follow

This repo is spec-driven (`specs/README.md`). You are between Phase 1.5 and
Phase 2. The remaining phases, **each gated on operator approval**:

1. `/plan`   → produce `specs/026-rig-agent-mesh/plan.md` (design)
2. `/tasks`  → produce `tasks.md` (ordered, verifiable tasks)
3. `/analyze` (optional but recommended here — big surface) → gap report
4. `/implement` → code + tests, tasks checked off
5. `/docsync` → English guide only (`docs/developers-guide-en.md`);
   **never touch the translated guides** (operator golden rule)

Do not skip gates. Do not start writing code because the spec "looks done".

## What /plan must decide (the hard parts)

Read spec.md first — these are the load-bearing design points it constrains:

- **`cobolt-agents` crate** (R1/R2): new workspace crate wrapping rig-core
  (pin the exact latest stable, `=x.y.z`; all rig types stay behind this
  adapter). Tokio runtime on background threads; egui-facing API is the same
  `spawn_* → mpsc::Receiver` pattern used today in `cobolt-ide/src/llm.rs`
  (`spawn_request` ~line 447 is the reference). No egui dependency in the
  crate.
- **Provider mapping** (R3): reuse the existing `LlmConfig`/`Provider` enum
  (`cobolt-ide/src/llm.rs` ~line 656) — Ollama, OpenAI-compatible, OpenRouter,
  HuggingFace, keyed clouds — onto rig clients. Gate rig provider features in
  Cargo so unused providers aren't compiled in.
- **Agent taxonomy** (R5/R5a): fixed domain specialists + **one statement
  agent per verb**, generated at build time from markdown+YAML-front-matter
  knowledge cards in a new `knowledge/verbs/` corpus (Q8 resolution). Plan the
  build step (build.rs or codegen) that turns front-matter into both the
  `check_capability` matrix and the per-verb agent cards, plus the CI
  cross-check against `docs/cobol85-verb-test-matrix.md`.
- **Pedantic tools** (R7/R7a): thin wrappers over `cobolt-lexer` (tokenize +
  `SourceFormat::detect`), `cobolt-parser::parse`, `cobolt-semantic::analyze`,
  the control/property schema (see `validate_form_source` in
  `cobolt-ide/src/app.rs` for the existing composition), and the data-binding
  guardian. `dry_run` spawns `rcrun` with a **new sandbox mode** (temp CWD,
  5 s kill, network/DB/system CALLs disabled) — model the spawn on
  `ExternalFormRun::spawn` in `cobolt-ide/src/form_runtime.rs`, which is the
  established child-process pattern (Run Form / Debug Form use it).
- **Gate loop** (R8): generate → verify → repair, max 3 iterations, verbatim
  diagnostics fed back. This is the核心 anti-hallucination mechanism; design
  it as a reusable combinator every code-producing agent goes through.
- **Retrieval** (R13): lexical-only BM25 + curated COBOL synonym table. No
  embeddings, no vector DB in v1 — do not let rig's vector-store features
  creep in.
- **Orchestrator** (R4/R6/R6a): specialists exposed as rig tools; routing must
  answer trivial prompts with zero fan-out; concurrency caps 2 local / 4
  cloud; ONE merged reply to chat, per-agent progress into the existing AI log
  (`AiLogKind` side-channel, llm.rs ~line 1435).
- **Deletion, not migration** (R19): the legacy single-agent request path in
  `cobolt-ide::llm` is removed in the same change. **The UI is the only
  compatibility surface** (prompt bar, chat bubbles, preview/approve, Agentic
  AI tree, settings page — all keep their current shape). Spec 025's UI
  acceptance criteria are the regression suite.

## Context you need before planning

- `specs/025-ai-dev-agent/spec.md` — the current agent's contract; its R14–R21
  define the `agentic_ai/` folder, prompt/skills seeding, memory rules. 026
  extends this; its Q6 deferral is what 026 resolves.
- `cobolt-ide/src/llm.rs` (~1500 lines) — everything being replaced/kept:
  keep `LlmConfig`, `Provider`, model listing, AI log; replace the request/
  chat plumbing.
- `cobolt-ide/src/agent.rs` — change-set parsing (`parse_change_set`) and the
  preview/apply path; the forms-rad specialist must keep emitting this
  vocabulary.
- A real project to test against: `/Users/emersonlopes/Documents/PowerDemo2`
  (has `agentic_ai/` with customized prompt + 4 skills).

## Operator conventions (hard rules, from memory + steering)

- Every release ships as a **fix**: z-bump only, CHANGELOG entry, commit
  message style "PowerRustCOBOL x.y.z - summary".
- **Ask before pushing** (push-window rule) and before forum posts. Forum:
  cobolforo.es f=97, no prefix, title ≤ ~50 chars, Windows-1252 — post via
  native browser form submit, sign "Anthropic Claude Codex Agent".
- Six-language i18n (`Tr` struct in `cobolt-ide/src/i18n.rs`) for every user
  string; English docs only.
- The operator runs the **installed bundle** (`~/Applications/
  PowerRustCOBOL.app`) — install release builds of BOTH `cobolt-ide` and
  `rcrun` there for testing.
- Any window/panel resize work: use the egui-resize-guardian agent; never size
  a child from available/max space (recurring self-inflation bug).
- Parallel sessions happen: **check `git log`/`version.rs` before assuming
  your version number** — the operator may have advanced it (this session
  collided twice; current at handoff: 1.28.17+).

## Suggested phase-2 sequencing (for plan.md, not binding)

1. Spike: `cobolt-agents` crate + rig pinned + one provider (Ollama) + one
   trivial agent answering through the existing UI — proves the
   runtime/channel bridge.
2. Pedantic tools (no LLM needed — pure wrappers + tests against
   `rcrun check` parity).
3. Orchestrator + forms-rad + 2 domain specialists, gate loop live, legacy
   path deleted.
4. Verb corpus + build-time statement agents + `check_capability`.
5. Retrieval index + synonym table; distilled memory; declared agents +
   hot-reload; traces (JSONL + AI log), optional OTel behind a feature.
