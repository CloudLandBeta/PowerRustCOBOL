<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 028 — Project agent database + Agent Manager modal

- **Status:** approved (operator, 2026-07-16 — via interactive mockup
  https://claude.ai/code/artifact/b57c65da-ed03-43c2-85fc-c3d5527d46b3)
- **Owner branch:** egui-035 (rides spec 027's branch until merge)

## Problem

The AI configuration is a fixed pair (primary model + optional pedantic
reviewer, spec-added 2026-07-16). The operator wants an **agent database**:
a project can define dozens of agents, each with its own model, prompt,
capabilities, and knowledge, and each primary agent may name a pedantic
companion that gates its responses.

## Requirements

- **R1 — Agent entity.** id (UUID, generated), name, model id (official),
  purpose, plus the four-branch structure: Core Instructions (agent prompt,
  steering files, policies), Capabilities (skills, tools, plugins, MCP server
  definitions), Knowledge (references, examples, domain docs), Runtime
  Configuration (model settings, permissions, routing rules, environment).
- **R2 — On-disk layout** (per agent, inside the project):
  `agentic_ai/<agent_name>/` containing `<agent_name>_prompt.md` (agent
  prompt, multi-line Markdown), `steering/`, `policies.md`, `skills/`,
  `mcp.json`, `knowledge/`, `agent.json` (identity + runtime config —
  **never** the API key).
- **R3 — Names.** Agent name is unique in the project and **immutable**
  after creation (it names the folder and prompt file). Rename = create new
  + delete old.
- **R4 — Keys.** API keys are asked per model, stored machine-global in the
  existing per-`provider::model` key map, never in the project.
- **R5 — Companion rule.** Every primary agent MAY have one pedantic
  companion agent that validates its responses. A primary and ITS companion
  must use different models; any other agents may share models freely.
- **R6 — Agent Manager modal.** Replaces the AI block in Project Settings
  with a summary row + "Manage agents…" button opening a modal:
  master-detail; left rail lists agents (companions nested under their
  primary, selected row emphasized, others dimmed); right side is ONE panel
  with collapsible sections Identity / Runtime Configuration / Core
  Instructions / Capabilities / Knowledge / On disk / Companion; footer
  validation line + Cancel/Apply/OK. Prompt editor is multi-line.
- **R7 — Seeding.** On first open of a project with no agents, seed from the
  current settings (existing connection → "Form Designer Agent"; reviewer
  model if configured → "Pedantic UI Agent" companion; the event-handler
  pedantic prompt seeds a "Pedantic COBOL Companion" when a reviewer
  exists). Nothing the user configured is lost.
- **R8 — Phase 1 runtime wiring.** The designer agent flow reads its model/
  prompt/companion from the DB entry named "Form Designer Agent" when
  present (legacy config as fallback). Full multi-agent orchestration
  (delegation, per-response companion gating in interactive flows) is
  **Phase 2** — out of scope here; the tandem benchmark loop from the
  fixed-pair feature keeps working unchanged.
- **R9 — i18n ×6** for all new UI strings.

## Acceptance criteria

- AC1: create/edit/delete agents in the modal; files appear/update under
  `agentic_ai/<name>/` exactly per R2; agent.json contains no key.
- AC2: duplicate or empty name rejected at creation; name read-only after.
- AC3: companion picker refuses the primary's own model (validation line).
- AC4: keys restore/clear per model exactly like the primary AI settings.
- AC5: seeding produces working agents from an existing configuration.
- AC6: designer agent uses the DB entry when present (R8), verified by a
  unit test on the config-resolution helper.
- AC7: tests green across the workspace; i18n test passes.
