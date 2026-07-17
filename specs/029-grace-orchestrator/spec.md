<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 029 — Grace, the PowerRustCOBOL Rig Orchestrator Agent

- **Status:** approved requirements (operator prompt, 2026-07-16, stored
  verbatim as Grace's agent prompt); phased delivery
- **Depends on:** spec 028 (agent database), spec 026/028 pedantic loops

## Identity (R1)

The orchestrator is named **Grace** — never referred to by any other name,
in code, UI, prompts, or docs. Grace is a singleton agent in the project
agent database (`agentic_ai/Grace/`), kind `orchestrator`, whose prompt file
`Grace_prompt.md` is seeded verbatim from the operator's Grace prompt (plus
the machine-readable tooling contract for plans/delegations/verdicts).
Grace has her own model + key (per-model store) — she is not free.

## Position in the rig architecture (R2)

`cobolt-agents` already ships a rig-core mesh: `Orchestrator` holding
`Specialist`s (FormsDesigner, CodeGenerator, EventBinder) with keyword
routing (`route_specialist`). Grace SUPERSEDES that layer:

- keyword routing becomes the **fallback** (used only when Grace is absent
  or disabled — e.g. no model configured);
- with Grace enabled, request handling becomes: Grace plans (structured
  output) → delegates task-by-task to DB agents (each mapped onto a rig
  specialist/completion with its own model, prompt, tools) → enforces
  companion reviews → runs bounded correction loops → integrates →
  assembles the final response;
- `MeshRequest` grows a `task: Option<TaskSpec>` delegation contract;
  specialists return `TaskResult` (structured), not bare text, when driven
  by Grace.

## Agent typing (R3) — DB + Agent Manager accommodate the new types

`AgentDef` gains `kind: orchestrator | specialist | pedantic` (serde
default `specialist` — existing manifests load unchanged) and
`specialization: String` (e.g. `form-design`, `cobol-events`, `cobol-dev`,
`security`, `documentation`; free-form for user agents). Rules:

- exactly one `orchestrator` per project, always named Grace;
  `ensure_grace()` creates/repairs it (idempotent), Delete is refused;
- `pedantic` agents are the only valid companions; kind is now STORED, so
  an unlinked pedantic no longer counts as an unreviewed primary (fixes the
  spec-028 orphan ambiguity);
- Grace selects agents by kind + specialization + declared capabilities
  (tools/skills), NEVER by name similarity (operator rule);
- the Agent Manager rail shows kind badges (Grace pinned first), the
  Identity section shows/edits `specialization`, and the mockup mirrors it.

## Orchestration semantics (R4 — from the operator prompt, binding)

The full operator prompt is normative and stored as Grace's agent prompt.
Key mechanics the runtime must implement:

- **Task decomposition**: unique task ids; per-task agent, objective,
  context, I/O contract, dependencies, review requirements, acceptance
  criteria, failure/retry conditions; no over-fragmentation.
- **Task states**: Pending, Ready, Running, AwaitingDependency,
  AwaitingReview, CorrectionRequired, Revalidating, Approved, Blocked,
  Failed, Completed — only Approved work reaches the final result.
- **Delegation contract** (`TaskSpec` → `TaskResult` structured JSON): what/
  why/may-touch/must-not-touch/authoritative instructions/expected output/
  evidence/reviewer/failure conditions → status/summary/resources/outputs/
  assumptions/warnings/validation/review status/references. "done" without
  evidence is rejected.
- **Review enforcement**: companion reviews are never optional; no agent
  approves its own work; full re-review after corrections; tracked verdicts
  and scores (reuses the spec-026/028 pedantic round/final JSON contract).
- **Correction loops**: bounded (default max 2 revisions, configurable on
  Grace's manifest); on exhaustion the task is Failed/Blocked, never
  silently completed.
- **Cross-agent integration**: identifier/name/event consistency checks;
  downstream revalidation when an approved artifact changes.
- **Parallelism**: only for independent tasks; never when identifiers or
  modified resources flow between tasks; no circular delegation.
- **Tool/MCP governance**: only declared tools (agent `mcp.json` + the IDE
  egui MCP endpoint); fabricated ops/ids are critical defects; tool
  evidence preserved.
- **Failure handling & completion criteria**: per the operator prompt,
  verbatim; no completion claims without execution + review evidence.
- **Observability**: workflow record (workflow id, task ids, agents, model
  per agent, tool calls, timestamps, transitions, review findings,
  correction cycles, verdicts) appended to the AI activity log and saved
  under `agentic_ai/Grace/runs/<workflow-id>.json`.

## Adjustments to existing work (R5)

- Seeding (spec 028 R7) also creates Grace + tags kinds (Form Designer
  Agent → specialist/form-design; companions → pedantic).
- `unreviewed_primaries` counts only enabled `specialist`s without a
  usable pedantic companion.
- The proficiency check keeps its direct primary↔companion tandem (no
  Grace involvement — it is a measurement, not a workflow).
- Routing text field stays as human documentation; Grace ignores it for
  selection (kind/specialization/capabilities only).

## Phases

- **Phase A (now):** DB typing + Grace singleton + prompt + seeding +
  Agent Manager/mockup accommodation + validations + tests.
- **Phase B:** rig runtime — TaskSpec/TaskResult types in cobolt-agents,
  Grace planner loop (structured outputs), review gates, bounded correction
  loops, workflow records; driven behind the existing mesh entry points.
- **Phase C:** interactive flows (designer pane, event modal) routed
  through Grace; egui-MCP tool execution for agents; delegation to the
  COBOL Event Handler Script Agent per the pedantic-UI contract.

## Resolved: generic endpoint wire resolution (2026-07-17)

`cobolt-agents` test `ollama_cloud_wrong_host_is_healed_and_openai_wire_chosen`
was failing (latent from the 1.30.x ollama-cloud healing on main; the crate
was absent from the spec-027 gate matrix): an Ollama-family provider forced
the native `/api/chat` wire even when the user's endpoint explicitly ended in
`/v1/chat/completions`. Fixed generically per operator direction — host
healing is now a provider-keyed table (`heal_endpoint_host`), and wire format
follows the most specific signal (`resolve_wire`): an explicit endpoint suffix
wins over the provider default, so any provider/model combination resolves
correctly. Two unit tests lock the provider-agnostic contract.

## Acceptance criteria

- AC1 (A): `ensure_grace` creates `agentic_ai/Grace/` with the verbatim
  prompt; second call is a no-op; Delete refused in the manager.
- AC2 (A): kinds serialize/deserialize; old manifests default to
  specialist; seeded DB has Grace + typed agents; exactly-one-orchestrator
  validation.
- AC3 (A): unreviewed warning ignores pedantic/orchestrator kinds.
- AC4 (A): Agent Manager + HTML mockup show kinds; Grace pinned; tests
  green workspace-wide.
- AC5 (B): a scripted two-task workflow (design→review→correct→approve)
  produces a workflow record with the mandated states and evidence.
- AC6 (C): a form request in the designer pane routes through Grace end to
  end with companion gates enforced.
